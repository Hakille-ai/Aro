//! Context Window Manager, Continuous Background Compactor, and Lossless Entity Extractor
//!
//! Enforces the 8,192 token context window ceiling across 5 partitions:
//! - System instructions: <= 800 tokens
//! - Long-term semantic knowledge: <= 1,600 tokens
//! - Chronological episodic summaries: <= 2,000 tokens
//! - Working memory buffer: <= 2,400 tokens (latest N <= 8 turns)
//! - Reserve completion headroom: <= 1,392 tokens
//!
//! Provides automated 10-turn continuous consolidation preventing context window saturation
//! across 100+ turns with lossless preservation of ports, URLs, secrets, architectural
//! decisions, and user rules.

use std::collections::HashSet;

use aro_core::{
    compute_initial_salience, estimate_message_tokens, estimate_tokens, AroError, AroResult,
    ChatMessage, ContextBudget, ContextEvictionReport, ContextTokenUsage, Episode, EpisodeSummary,
    LongTermMemory, MemoryCategory, MessageRole, DEFAULT_MEMORY_SCOPE, MEMORY_STATUS_APPROVED,
};
use aro_memory::SqliteMemoryStore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const COMPACTION_TURN_INTERVAL: usize = 10;
pub const COMPACTION_TOKEN_THRESHOLD: usize = 2400;

/// Fully assembled and bounded context package.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssembledContext {
    pub system_prompt: String,
    pub semantic_memories: Vec<LongTermMemory>,
    pub episodic_summaries: Vec<EpisodeSummary>,
    pub working_messages: Vec<ChatMessage>,
    pub token_usage: ContextTokenUsage,
    pub eviction_report: ContextEvictionReport,
}

/// Adaptive context window manager strictly enforcing partition budgets
/// and the 8,192 total context ceiling.
#[derive(Debug, Clone, Default)]
pub struct ContextWindowManager {
    budget: ContextBudget,
}

impl ContextWindowManager {
    pub fn new(budget: ContextBudget) -> Self {
        Self { budget }
    }

    pub fn budget(&self) -> &ContextBudget {
        &self.budget
    }

    /// Evaluates whether uncompacted turns or working tokens exceed consolidation thresholds.
    pub fn needs_compaction(&self, total_turns: usize, working_tokens: usize) -> bool {
        total_turns >= COMPACTION_TURN_INTERVAL || working_tokens >= self.budget.working_budget
    }

    /// Fits system prompt into system budget (<= 800 tokens).
    pub fn fit_system_prompt(&self, prompt: &str) -> AroResult<(String, usize)> {
        let tokens = estimate_tokens(prompt);
        if tokens > self.budget.system_budget {
            return Err(AroError::Configuration(format!(
                "System prompt exceeds budget: {} > {}",
                tokens, self.budget.system_budget
            )));
        }
        Ok((prompt.to_string(), tokens))
    }

    /// Fits semantic memories within budget (<= 1600 tokens) using priority:
    /// 1. pinned first
    /// 2. descending salience
    /// 3. descending created_at
    pub fn fit_semantic_memories(
        &self,
        candidates: &[LongTermMemory],
    ) -> (Vec<LongTermMemory>, usize, usize) {
        let mut sorted = candidates.to_vec();
        sorted.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| {
                    b.salience
                        .partial_cmp(&a.salience)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| b.created_at.cmp(&a.created_at))
        });

        let mut retained = Vec::new();
        let mut used_tokens = 0usize;

        for memory in sorted {
            let mem_tokens = estimate_tokens(&memory.content);
            if used_tokens + mem_tokens <= self.budget.semantic_budget {
                used_tokens += mem_tokens;
                retained.push(memory);
            }
        }

        let evicted = candidates.len().saturating_sub(retained.len());
        (retained, used_tokens, evicted)
    }

    /// Fits episodic summaries into budget (<= 2000 tokens) preserving chronological order.
    pub fn fit_episodic_summaries(
        &self,
        candidates: &[EpisodeSummary],
    ) -> (Vec<EpisodeSummary>, usize, usize) {
        let mut sorted = candidates.to_vec();
        sorted.sort_by_key(|ep| ep.turn_start);

        let mut retained = Vec::new();
        let mut used_tokens = 0usize;

        for ep in sorted {
            let ep_text = ep.format_for_prompt();
            let ep_tokens = estimate_tokens(&ep_text);
            if used_tokens + ep_tokens <= self.budget.episodic_budget {
                used_tokens += ep_tokens;
                retained.push(ep);
            }
        }

        let evicted = candidates.len().saturating_sub(retained.len());
        (retained, used_tokens, evicted)
    }

    /// Fits working memory messages (N <= 8 turns, <= 2400 tokens) using
    /// reverse recency greedy window.
    pub fn fit_working_messages(
        &self,
        messages: &[ChatMessage],
    ) -> (Vec<ChatMessage>, usize, usize) {
        let mut retained_rev = Vec::new();
        let mut used_tokens = 0usize;

        // Traverse backwards from newest to oldest
        for msg in messages.iter().rev() {
            if retained_rev.len() >= self.budget.max_working_turns {
                break;
            }
            let msg_tokens = estimate_message_tokens(msg);
            if used_tokens + msg_tokens <= self.budget.working_budget {
                used_tokens += msg_tokens;
                retained_rev.push(msg.clone());
            } else {
                break;
            }
        }

        retained_rev.reverse();
        let evicted = messages.len().saturating_sub(retained_rev.len());
        (retained_rev, used_tokens, evicted)
    }

    /// Assembles full context pack, strictly validating all invariants and the 8,192 ceiling.
    pub fn assemble_context(
        &self,
        system_prompt: &str,
        semantic_candidates: &[LongTermMemory],
        episodic_candidates: &[EpisodeSummary],
        working_messages: &[ChatMessage],
    ) -> AroResult<AssembledContext> {
        let (fitted_sys, sys_tokens) = self.fit_system_prompt(system_prompt)?;
        let (fitted_sem, sem_tokens, sem_evicted) =
            self.fit_semantic_memories(semantic_candidates);
        let (fitted_epi, epi_tokens, epi_evicted) =
            self.fit_episodic_summaries(episodic_candidates);
        let (fitted_work, work_tokens, work_evicted) =
            self.fit_working_messages(working_messages);

        let total_input_tokens = sys_tokens + sem_tokens + epi_tokens + work_tokens;
        let total_tokens = total_input_tokens + self.budget.reserve_budget;

        if total_tokens > self.budget.total_ceiling {
            return Err(AroError::Configuration(format!(
                "Context ceiling violation: total {} exceeds ceiling {}",
                total_tokens, self.budget.total_ceiling
            )));
        }

        let token_usage = ContextTokenUsage {
            system_tokens: sys_tokens,
            semantic_tokens: sem_tokens,
            episodic_tokens: epi_tokens,
            working_tokens: work_tokens,
            total_input_tokens,
            reserve_tokens: self.budget.reserve_budget,
            total_tokens,
        };

        let eviction_report = ContextEvictionReport {
            semantic_candidates: semantic_candidates.len(),
            semantic_retained: fitted_sem.len(),
            semantic_evicted: sem_evicted,
            episodic_candidates: episodic_candidates.len(),
            episodic_retained: fitted_epi.len(),
            episodic_evicted: epi_evicted,
            working_candidates: working_messages.len(),
            working_retained: fitted_work.len(),
            working_evicted: work_evicted,
        };

        Ok(AssembledContext {
            system_prompt: fitted_sys,
            semantic_memories: fitted_sem,
            episodic_summaries: fitted_epi,
            working_messages: fitted_work,
            token_usage,
            eviction_report,
        })
    }

    /// Formats semantic and episodic context directly into composite system instructions.
    pub fn render_system_prompt(&self, assembled: &AssembledContext) -> String {
        let mut out = assembled.system_prompt.clone();

        if !assembled.semantic_memories.is_empty() {
            out.push_str("\n\n## Long-Term Semantic Knowledge:\n");
            for mem in &assembled.semantic_memories {
                let pin_badge = if mem.pinned { " [PINNED]" } else { "" };
                out.push_str(&format!("- [{}{}]: {}\n", mem.category, pin_badge, mem.content));
            }
        }

        if !assembled.episodic_summaries.is_empty() {
            out.push_str("\n\n## Chronological Episodic History:\n");
            for ep in &assembled.episodic_summaries {
                out.push_str(&ep.format_for_prompt());
                out.push('\n');
            }
        }

        out
    }
}

// ============================================================================
// Continuous Background Consolidation & Compactor
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionResult {
    pub episode: Episode,
    pub extracted_memories: Vec<LongTermMemory>,
    pub turns_compacted: usize,
    pub tokens_before: usize,
    pub tokens_after: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactionTriggerReason {
    TurnIntervalReached,     // uncompacted_turns >= 10
    TokenThresholdExceeded,  // uncompacted_tokens >= 2400
    ExplicitFlush,           // manual consolidation request
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactionDecision {
    Skip {
        reason: String,
    },
    Trigger {
        turn_start: usize,
        turn_end: usize,
        message_ids: Vec<Uuid>,
        estimated_tokens: usize,
        reason: CompactionTriggerReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionState {
    pub conversation_id: Uuid,
    pub last_compacted_turn: usize,
    pub current_turn: usize,
    pub uncompacted_turns: usize,
    pub uncompacted_tokens: usize,
}

impl CompactionState {
    /// Recovers state by querying the latest episode from SQLite.
    pub fn recover(
        store: &SqliteMemoryStore,
        conversation_id: Uuid,
        messages: &[ChatMessage],
    ) -> AroResult<Self> {
        let latest_episode = store.get_latest_episode(conversation_id)?;
        let last_compacted_turn = latest_episode.map(|ep| ep.turn_end).unwrap_or(0);

        let user_message_count = messages
            .iter()
            .filter(|m| m.role == MessageRole::User)
            .count();
        let current_turn = user_message_count.max(if messages.is_empty() { 0 } else { 1 });

        let uncompacted_turns = current_turn.saturating_sub(last_compacted_turn);

        // Count tokens for messages beyond last_compacted_turn
        let uncompacted_messages = slice_uncompacted_messages(messages, last_compacted_turn);
        let uncompacted_tokens = uncompacted_messages
            .iter()
            .map(|m| {
                m.token_estimate
                    .map(|t| t as usize)
                    .unwrap_or_else(|| estimate_tokens(&m.content))
            })
            .sum();

        Ok(Self {
            conversation_id,
            last_compacted_turn,
            current_turn,
            uncompacted_turns,
            uncompacted_tokens,
        })
    }

    /// Evaluates whether compaction should trigger.
    pub fn evaluate(&self, messages: &[ChatMessage]) -> CompactionDecision {
        self.evaluate_with_interval(messages, COMPACTION_TURN_INTERVAL)
    }

    pub fn evaluate_with_interval(&self, messages: &[ChatMessage], interval: usize) -> CompactionDecision {
        if self.uncompacted_turns == 0 {
            return CompactionDecision::Skip {
                reason: "No uncompacted turns available".to_string(),
            };
        }

        let interval = interval.max(2);
        let turn_start = self.last_compacted_turn + 1;

        if self.uncompacted_turns >= interval {
            let turn_end = turn_start + interval - 1;
            let slice = slice_turn_range(messages, turn_start, turn_end);
            let message_ids = slice.iter().map(|m| m.id).collect();
            let estimated_tokens = slice
                .iter()
                .map(|m| {
                    m.token_estimate
                        .map(|t| t as usize)
                        .unwrap_or_else(|| estimate_tokens(&m.content))
                })
                .sum();

            return CompactionDecision::Trigger {
                turn_start,
                turn_end,
                message_ids,
                estimated_tokens,
                reason: CompactionTriggerReason::TurnIntervalReached,
            };
        }

        if self.uncompacted_tokens >= COMPACTION_TOKEN_THRESHOLD && self.uncompacted_turns > 0 {
            let turn_end = self.current_turn;
            let slice = slice_turn_range(messages, turn_start, turn_end);
            let message_ids = slice.iter().map(|m| m.id).collect();

            return CompactionDecision::Trigger {
                turn_start,
                turn_end,
                message_ids,
                estimated_tokens: self.uncompacted_tokens,
                reason: CompactionTriggerReason::TokenThresholdExceeded,
            };
        }

        CompactionDecision::Skip {
            reason: format!(
                "Below thresholds: {}/{} turns, {}/{} tokens",
                self.uncompacted_turns,
                interval,
                self.uncompacted_tokens,
                COMPACTION_TOKEN_THRESHOLD
            ),
        }
    }
}

pub trait ContextCompactor: Send + Sync {
    fn should_compact(&self, turn_count: usize, estimated_tokens: usize) -> bool;
    fn compact_turns(
        &self,
        conversation_id: Uuid,
        turns: &[ChatMessage],
    ) -> AroResult<CompactionResult>;
}

/// Continuous Compactor providing automated incremental background compaction.
#[derive(Clone)]
pub struct ContinuousCompactor {
    store: SqliteMemoryStore,
    compaction_interval: usize,
}

impl ContinuousCompactor {
    pub fn new(store: SqliteMemoryStore) -> Self {
        Self {
            store,
            compaction_interval: COMPACTION_TURN_INTERVAL,
        }
    }

    pub fn new_with_interval(store: SqliteMemoryStore, compaction_interval: usize) -> Self {
        Self {
            store,
            compaction_interval: compaction_interval.max(2),
        }
    }

    /// Evaluates compaction for a conversation and executes if thresholds are met.
    pub fn consolidate_if_needed(
        &self,
        conversation_id: Uuid,
    ) -> AroResult<Option<CompactionResult>> {
        let messages = self.store.list_messages(conversation_id)?;
        if messages.is_empty() {
            return Ok(None);
        }

        let state = CompactionState::recover(&self.store, conversation_id, &messages)?;
        match state.evaluate_with_interval(&messages, self.compaction_interval) {
            CompactionDecision::Skip { .. } => Ok(None),
            CompactionDecision::Trigger {
                turn_start,
                turn_end,
                ..
            } => {
                let slice = slice_turn_range(&messages, turn_start, turn_end);
                let result = self.compact_slice(conversation_id, turn_start, turn_end, &slice)?;
                Ok(Some(result))
            }
        }
    }

    /// Force consolidation of all uncompacted turns up to current.
    pub fn force_consolidate(
        &self,
        conversation_id: Uuid,
    ) -> AroResult<Option<CompactionResult>> {
        let messages = self.store.list_messages(conversation_id)?;
        if messages.is_empty() {
            return Ok(None);
        }

        let state = CompactionState::recover(&self.store, conversation_id, &messages)?;
        if state.uncompacted_turns == 0 {
            return Ok(None);
        }

        let turn_start = state.last_compacted_turn + 1;
        let turn_end = state.current_turn;
        let slice = slice_turn_range(&messages, turn_start, turn_end);
        let result = self.compact_slice(conversation_id, turn_start, turn_end, &slice)?;
        Ok(Some(result))
    }

    fn compact_slice(
        &self,
        conversation_id: Uuid,
        turn_start: usize,
        turn_end: usize,
        slice: &[ChatMessage],
    ) -> AroResult<CompactionResult> {
        if slice.is_empty() {
            return Err(AroError::Configuration(
                "cannot compact an empty message slice".to_string(),
            ));
        }

        let tokens_before: usize = slice
            .iter()
            .map(|m| {
                m.token_estimate
                    .map(|t| t as usize)
                    .unwrap_or_else(|| estimate_tokens(&m.content))
            })
            .sum();

        let extracted =
            LosslessEntityExtractor::extract(conversation_id, turn_start, turn_end, slice);

        let episode = Episode::new(
            conversation_id,
            turn_start,
            turn_end,
            extracted.summary,
            extracted.key_decisions,
            extracted.entities,
            tokens_before,
        );

        // 1. Atomically persist the episode into SQLite
        self.store.store_episode(&episode)?;

        // 2. Persist extracted high-salience facts into long-term memories
        let mut persisted_memories = Vec::new();
        for memory in &extracted.proposed_memories {
            let saved = self.store.upsert_memory(memory)?;
            persisted_memories.push(saved);
        }

        let tokens_after =
            estimate_tokens(&EpisodeSummary::from(&episode).format_for_prompt());

        Ok(CompactionResult {
            episode,
            extracted_memories: persisted_memories,
            turns_compacted: turn_end.saturating_sub(turn_start) + 1,
            tokens_before,
            tokens_after,
        })
    }
}

impl ContextCompactor for ContinuousCompactor {
    fn should_compact(&self, turn_count: usize, estimated_tokens: usize) -> bool {
        turn_count >= COMPACTION_TURN_INTERVAL || estimated_tokens >= COMPACTION_TOKEN_THRESHOLD
    }

    fn compact_turns(
        &self,
        conversation_id: Uuid,
        turns: &[ChatMessage],
    ) -> AroResult<CompactionResult> {
        if turns.is_empty() {
            return Err(AroError::Configuration(
                "turns slice cannot be empty".into(),
            ));
        }
        let latest = self.store.get_latest_episode(conversation_id)?;
        let turn_start = latest.map(|e| e.turn_end + 1).unwrap_or(1);
        let user_turns = turns.iter().filter(|m| m.role == MessageRole::User).count();
        let turn_end = turn_start + user_turns.saturating_sub(1);
        self.compact_slice(conversation_id, turn_start, turn_end, turns)
    }
}

// ============================================================================
// Lossless Entity & Decision Extractor Implementation
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedCompactionData {
    pub summary: String,
    pub key_decisions: Vec<String>,
    pub entities: Vec<String>,
    pub proposed_memories: Vec<LongTermMemory>,
}

pub struct LosslessEntityExtractor;

impl LosslessEntityExtractor {
    pub fn extract(
        conversation_id: Uuid,
        turn_start: usize,
        turn_end: usize,
        messages: &[ChatMessage],
    ) -> ExtractedCompactionData {
        let mut entities_set = HashSet::new();
        let mut entities = Vec::new();

        let mut decisions_set = HashSet::new();
        let mut key_decisions = Vec::new();

        let mut proposed_memories = Vec::new();
        let mut turn_summaries = Vec::new();

        let mut current_turn_idx = turn_start;
        let mut last_user_content: Option<String> = None;
        let mut last_assistant_content: Option<String> = None;

        for msg in messages {
            let content = msg.content.trim();
            if content.is_empty() {
                continue;
            }

            // 1. Extract network ports & endpoints
            for port_entity in extract_ports(content) {
                if entities_set.insert(port_entity.clone()) {
                    entities.push(port_entity);
                }
            }

            // 2. Extract URLs and URIs
            for url in extract_urls_and_uris(content) {
                if entities_set.insert(url.clone()) {
                    entities.push(url);
                }
            }

            // 3. Extract Configs and Secret Keys
            for config in extract_configs_and_keys(content) {
                if entities_set.insert(config.clone()) {
                    entities.push(config);
                }
            }

            // 4. Extract Architectural Decisions
            for decision in extract_decisions(content) {
                if decisions_set.insert(decision.clone()) {
                    key_decisions.push(decision.clone());

                    // Create high-salience LongTermMemory
                    let mut mem = LongTermMemory::new(decision.clone(), Some(conversation_id));
                    mem.category = MemoryCategory::Technical.as_str().to_string();
                    mem.scope = DEFAULT_MEMORY_SCOPE.to_string();
                    mem.status = MEMORY_STATUS_APPROVED.to_string();
                    mem.source_message_ids = vec![msg.id];
                    mem.pinned = false;
                    mem.salience =
                        compute_initial_salience(&decision, false, MemoryCategory::Technical);
                    proposed_memories.push(mem);
                }
            }

            // 5. Extract User Rules, Directives & Preferences
            for rule in extract_user_rules(content) {
                if decisions_set.insert(rule.clone()) {
                    key_decisions.push(rule.clone());

                    let is_pref = rule.to_lowercase().contains("prefer")
                        || rule.to_lowercase().contains("préfère");
                    let category = if is_pref {
                        MemoryCategory::Preference
                    } else {
                        MemoryCategory::System
                    };

                    let mut mem = LongTermMemory::new(rule.clone(), Some(conversation_id));
                    mem.category = category.as_str().to_string();
                    mem.scope = DEFAULT_MEMORY_SCOPE.to_string();
                    mem.status = MEMORY_STATUS_APPROVED.to_string();
                    mem.source_message_ids = vec![msg.id];
                    mem.pinned = true; // Rules are pinned to exempt from decay!
                    mem.salience = compute_initial_salience(&rule, true, category);
                    proposed_memories.push(mem);
                }
            }

            // 6. Track turns for summary narrative
            if msg.role == MessageRole::User {
                if let (Some(u), Some(a)) = (last_user_content.take(), last_assistant_content.take()) {
                    turn_summaries.push(format!(
                        "[Turn {}] User: \"{}\" -> Assistant: \"{}\"",
                        current_turn_idx,
                        truncate_chars(&u, 80),
                        truncate_chars(&a, 80)
                    ));
                    current_turn_idx += 1;
                }
                last_user_content = Some(content.to_string());
            } else if msg.role == MessageRole::Assistant {
                last_assistant_content = Some(content.to_string());
            }
        }

        if let (Some(u), Some(a)) = (last_user_content, last_assistant_content) {
            turn_summaries.push(format!(
                "[Turn {}] User: \"{}\" -> Assistant: \"{}\"",
                current_turn_idx,
                truncate_chars(&u, 80),
                truncate_chars(&a, 80)
            ));
        }

        let summary = if turn_summaries.is_empty() {
            format!(
                "Consolidated turns {}-{} with {} messages.",
                turn_start,
                turn_end,
                messages.len()
            )
        } else {
            format!(
                "Consolidated turns {}-{}:\n{}",
                turn_start,
                turn_end,
                turn_summaries.join("\n")
            )
        };

        ExtractedCompactionData {
            summary,
            key_decisions,
            entities,
            proposed_memories,
        }
    }
}

// ============================================================================
// Deterministic String Parsing Helpers
// ============================================================================

fn truncate_chars(s: &str, max_len: usize) -> String {
    let s_clean = s.replace('\n', " ");
    if s_clean.chars().count() <= max_len {
        s_clean
    } else {
        format!("{}...", s_clean.chars().take(max_len).collect::<String>())
    }
}

pub fn slice_turn_range(
    messages: &[ChatMessage],
    turn_start: usize,
    turn_end: usize,
) -> Vec<ChatMessage> {
    let mut current_turn = 0;
    let mut result = Vec::new();

    for msg in messages {
        if msg.role == MessageRole::User {
            current_turn += 1;
        }
        let effective_turn = current_turn.max(1);
        if effective_turn >= turn_start && effective_turn <= turn_end {
            result.push(msg.clone());
        }
    }
    result
}

pub fn slice_uncompacted_messages(
    messages: &[ChatMessage],
    last_compacted_turn: usize,
) -> Vec<ChatMessage> {
    let mut current_turn = 0;
    let mut result = Vec::new();

    for msg in messages {
        if msg.role == MessageRole::User {
            current_turn += 1;
        }
        let effective_turn = current_turn.max(1);
        if effective_turn > last_compacted_turn {
            result.push(msg.clone());
        }
    }
    result
}

pub fn extract_ports(input: &str) -> Vec<String> {
    let mut results = Vec::new();
    let words: Vec<&str> = input.split_whitespace().collect();

    for (i, word) in words.iter().enumerate() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != ':' && c != '.');

        // Pattern: port 8080, port: 8080, port=8080
        if clean.eq_ignore_ascii_case("port")
            || clean.eq_ignore_ascii_case("port:")
            || clean.eq_ignore_ascii_case("ports")
        {
            if let Some(next_word) = words.get(i + 1) {
                let p_str = next_word.trim_matches(|c: char| !c.is_numeric());
                if let Ok(port) = p_str.parse::<u16>() {
                    if port > 0 {
                        results.push(format!("Port: {}", port));
                    }
                }
            }
        } else if clean.to_ascii_lowercase().starts_with("port=")
            || clean.to_ascii_lowercase().starts_with("port:")
        {
            let val = clean.split(['=', ':']).nth(1).unwrap_or("");
            let p_str = val.trim_matches(|c: char| !c.is_numeric());
            if let Ok(port) = p_str.parse::<u16>() {
                if port > 0 {
                    results.push(format!("Port: {}", port));
                }
            }
        } else if clean.contains(':') {
            // Endpoint: localhost:8080 or 127.0.0.1:8080
            let parts: Vec<&str> = clean.split(':').collect();
            if parts.len() == 2 {
                let host = parts[0];
                let port_str = parts[1].trim_matches(|c: char| !c.is_numeric());
                if (host.eq_ignore_ascii_case("localhost")
                    || host == "127.0.0.1"
                    || host == "0.0.0.0"
                    || host.ends_with(".internal"))
                    && !port_str.is_empty()
                {
                    if let Ok(port) = port_str.parse::<u16>() {
                        if port > 0 {
                            results.push(format!("Endpoint: {}:{}", host, port));
                        }
                    }
                }
            }
        }
    }

    results
}

pub fn extract_urls_and_uris(input: &str) -> Vec<String> {
    let mut results = Vec::new();
    let words = input.split_whitespace();

    for word in words {
        let trimmed = word.trim_matches(|c: char| {
            c == '(' || c == ')' || c == '[' || c == ']' || c == '<' || c == '>'
                || c == '"' || c == '\'' || c == ',' || c == ';' || c == '.'
        });

        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("ws://")
            || lower.starts_with("wss://")
            || lower.starts_with("postgres://")
            || lower.starts_with("postgresql://")
            || lower.starts_with("mysql://")
            || lower.starts_with("sqlite://")
            || lower.starts_with("mongodb://")
            || lower.starts_with("redis://")
        {
            results.push(format!("URL: {}", trimmed));
        }
    }

    results
}

pub fn extract_configs_and_keys(input: &str) -> Vec<String> {
    let mut results = Vec::new();
    let words = input.split_whitespace();

    for word in words {
        let trimmed = word.trim_matches(|c: char| {
            c == '(' || c == ')' || c == '[' || c == ']' || c == '<' || c == '>'
                || c == '"' || c == '\'' || c == ',' || c == ';'
        });

        // Match KEY=VALUE configurations
        if let Some(eq_idx) = trimmed.find('=') {
            let key = &trimmed[..eq_idx];
            let val = &trimmed[eq_idx + 1..];
            if key.len() >= 3
                && key.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
                && !val.is_empty()
            {
                results.push(format!("Config: {}", trimmed));
                continue;
            }
        }

        // Match secret key patterns
        if (trimmed.starts_with("sk-") && trimmed.len() >= 20)
            || (trimmed.starts_with("ghp_") && trimmed.len() >= 30)
            || (trimmed.starts_with("AKIA") && trimmed.len() >= 16)
        {
            results.push(format!("SecretKey: {}", trimmed));
        }
    }

    results
}

pub fn extract_decisions(input: &str) -> Vec<String> {
    let mut results = Vec::new();
    let sentences = split_sentences(input);

    let decision_markers = [
        "we decided to",
        "decided to",
        "decision:",
        "architecture decision:",
        "selected",
        "use ",
        "switched to",
        "adopted",
        "standardized on",
        "nous avons décidé de",
        "décision :",
        "choix d'architecture :",
        "on utilisera",
        "adopté",
        "retenu",
    ];

    for sentence in sentences {
        let lower = sentence.to_ascii_lowercase();
        let is_decision = decision_markers.iter().any(|&m| lower.contains(m));
        if is_decision && sentence.len() >= 10 {
            results.push(format!("Decision: {}", sentence.trim()));
        }
    }

    results
}

pub fn extract_user_rules(input: &str) -> Vec<String> {
    let mut results = Vec::new();
    let sentences = split_sentences(input);

    let rule_markers = [
        "always",
        "never",
        "must",
        "must not",
        "shall",
        "shall not",
        "rule:",
        "constraint:",
        "forbidden",
        "mandatory",
        "strictly",
        "require",
        "remember that",
        "remember:",
        "don't forget",
        "do not forget",
        "keep in mind",
        "i prefer",
        "preference:",
        "we prefer",
        "toujours",
        "jamais",
        "il faut",
        "ne faut jamais",
        "règle :",
        "contrainte :",
        "interdit",
        "obligatoire",
        "strictement",
        "souviens-toi",
        "n'oublie pas",
        "je préfère",
    ];

    for sentence in sentences {
        let lower = sentence.to_ascii_lowercase();
        let is_rule = rule_markers.iter().any(|&m| lower.contains(m));
        if is_rule && sentence.len() >= 10 {
            let is_pref = lower.contains("prefer") || lower.contains("préfère");
            let prefix = if is_pref { "Preference: " } else { "Rule: " };
            results.push(format!("{}{}", prefix, sentence.trim()));
        }
    }

    results
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            if !current.trim().is_empty() {
                sentences.push(current.trim().to_string());
                current.clear();
            }
            continue;
        }

        for c in trimmed_line.chars() {
            current.push(c);
            if c == '.' || c == '!' || c == '?' || c == '\n' {
                let s = current.trim().to_string();
                if s.len() >= 10 {
                    sentences.push(s);
                }
                current.clear();
            }
        }

        if !current.trim().is_empty() {
            current.push(' ');
        }
    }

    if !current.trim().is_empty() {
        sentences.push(current.trim().to_string());
    }

    sentences
}
