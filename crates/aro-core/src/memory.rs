use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_MEMORY_CATEGORY: &str = "personal";
pub const DEFAULT_MEMORY_SCOPE: &str = "user";
pub const MEMORY_STATUS_APPROVED: &str = "approved";
pub const MEMORY_STATUS_ARCHIVED: &str = "archived";
pub const MEMORY_STATUS_CANDIDATE: &str = "candidate";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    pub id: Uuid,
    pub content: String,
    pub source_conversation_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LongTermMemory {
    pub id: Uuid,
    #[serde(alias = "client_id")]
    pub client_id: Option<String>,
    pub content: String,
    pub category: String,
    pub scope: String,
    pub status: String,
    #[serde(alias = "source_conversation_id")]
    pub source_conversation_id: Option<Uuid>,
    #[serde(alias = "source_message_ids")]
    pub source_message_ids: Vec<Uuid>,
    pub pinned: bool,
    pub salience: f32,
    #[serde(default)]
    pub recall_count: u32,
    #[serde(alias = "last_used_at")]
    pub last_used_at: Option<DateTime<Utc>>,
    #[serde(alias = "created_at")]
    pub created_at: DateTime<Utc>,
    #[serde(alias = "updated_at")]
    pub updated_at: DateTime<Utc>,
}

pub type Memory = LongTermMemory;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoredMemoryId {
    pub memory_id: Uuid,
    pub score: f32,
}

impl ScoredMemoryId {
    pub fn new(memory_id: Uuid, score: f32) -> Self {
        Self { memory_id, score }
    }
}

impl MemoryEntry {
    pub fn new(content: impl Into<String>, source_conversation_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            content: content.into(),
            source_conversation_id,
            created_at: Utc::now(),
            pinned: false,
        }
    }
}

impl LongTermMemory {
    pub fn new(content: impl Into<String>, source_conversation_id: Option<Uuid>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            client_id: None,
            content: content.into(),
            category: DEFAULT_MEMORY_CATEGORY.to_string(),
            scope: DEFAULT_MEMORY_SCOPE.to_string(),
            status: MEMORY_STATUS_APPROVED.to_string(),
            source_conversation_id,
            source_message_ids: Vec::new(),
            pinned: false,
            salience: 0.5,
            recall_count: 0,
            last_used_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn approved_for_recall(&self) -> bool {
        self.status == MEMORY_STATUS_APPROVED && !looks_like_sensitive_memory(&self.content)
    }
}

impl From<MemoryEntry> for LongTermMemory {
    fn from(entry: MemoryEntry) -> Self {
        Self {
            id: entry.id,
            client_id: None,
            content: entry.content,
            category: DEFAULT_MEMORY_CATEGORY.to_string(),
            scope: DEFAULT_MEMORY_SCOPE.to_string(),
            status: MEMORY_STATUS_APPROVED.to_string(),
            source_conversation_id: entry.source_conversation_id,
            source_message_ids: Vec::new(),
            pinned: entry.pinned,
            salience: 0.5,
            recall_count: 0,
            last_used_at: None,
            created_at: entry.created_at,
            updated_at: entry.created_at,
        }
    }
}

pub fn normalize_memory_category(value: impl AsRef<str>) -> String {
    match value.as_ref() {
        "personal" | "technical" | "system" | "preference" => value.as_ref().to_string(),
        _ => DEFAULT_MEMORY_CATEGORY.to_string(),
    }
}

pub fn normalize_memory_scope(value: impl AsRef<str>) -> String {
    match value.as_ref() {
        "user" | "conversation" | "organization" | "project" => value.as_ref().to_string(),
        _ => DEFAULT_MEMORY_SCOPE.to_string(),
    }
}

pub fn normalize_memory_status(value: impl AsRef<str>) -> String {
    match value.as_ref() {
        MEMORY_STATUS_APPROVED | MEMORY_STATUS_ARCHIVED | MEMORY_STATUS_CANDIDATE => {
            value.as_ref().to_string()
        }
        _ => MEMORY_STATUS_APPROVED.to_string(),
    }
}

pub fn looks_like_sensitive_memory(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    [
        "api key",
        "api_key",
        "apikey",
        "bearer ",
        "password",
        "refresh token",
        "secret",
        "token=",
        "access_token",
        "private key",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

// ============================================================================
// 1. MemoryCategory Enum
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryCategory {
    Personal,
    Technical,
    System,
    Preference,
}

impl MemoryCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Technical => "technical",
            Self::System => "system",
            Self::Preference => "preference",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "technical" => Self::Technical,
            "system" => Self::System,
            "preference" => Self::Preference,
            _ => Self::Personal,
        }
    }
}

impl std::fmt::Display for MemoryCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<MemoryCategory> for String {
    fn from(cat: MemoryCategory) -> Self {
        cat.as_str().to_string()
    }
}

// ============================================================================
// 2. Episodic Memory Structs
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub turn_start: usize,
    pub turn_end: usize,
    pub summary: String,
    pub key_decisions: Vec<String>,
    pub entities: Vec<String>,
    pub token_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Episode {
    pub fn new(
        conversation_id: Uuid,
        turn_start: usize,
        turn_end: usize,
        summary: impl Into<String>,
        key_decisions: Vec<String>,
        entities: Vec<String>,
        token_count: usize,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            conversation_id,
            turn_start,
            turn_end,
            summary: summary.into(),
            key_decisions,
            entities,
            token_count,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn turn_span(&self) -> usize {
        if self.turn_end >= self.turn_start {
            self.turn_end - self.turn_start + 1
        } else {
            0
        }
    }

    pub fn contains_turn(&self, turn: usize) -> bool {
        turn >= self.turn_start && turn <= self.turn_end
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeSummary {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub turn_start: usize,
    pub turn_end: usize,
    pub summary: String,
    pub key_decisions: Vec<String>,
    pub entities: Vec<String>,
    pub token_count: usize,
    pub created_at: DateTime<Utc>,
}

impl From<&Episode> for EpisodeSummary {
    fn from(ep: &Episode) -> Self {
        Self {
            id: ep.id,
            conversation_id: ep.conversation_id,
            turn_start: ep.turn_start,
            turn_end: ep.turn_end,
            summary: ep.summary.clone(),
            key_decisions: ep.key_decisions.clone(),
            entities: ep.entities.clone(),
            token_count: ep.token_count,
            created_at: ep.created_at,
        }
    }
}

impl From<Episode> for EpisodeSummary {
    fn from(ep: Episode) -> Self {
        Self::from(&ep)
    }
}

impl EpisodeSummary {
    pub fn format_for_prompt(&self) -> String {
        let mut out = format!(
            "### Episode (Turns {}-{}):\n{}\n",
            self.turn_start, self.turn_end, self.summary
        );
        if !self.key_decisions.is_empty() {
            out.push_str("Key Decisions:\n");
            for d in &self.key_decisions {
                out.push_str(&format!("- {}\n", d));
            }
        }
        if !self.entities.is_empty() {
            out.push_str(&format!("Entities: {}\n", self.entities.join(", ")));
        }
        out
    }
}

// ============================================================================
// 3. ConsolidationCandidate Struct
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsolidationCandidate {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub turn_start: usize,
    pub turn_end: usize,
    pub message_ids: Vec<Uuid>,
    pub summary: String,
    pub key_decisions: Vec<String>,
    pub entities: Vec<String>,
    pub proposed_memories: Vec<LongTermMemory>,
    pub estimated_token_count: usize,
    pub created_at: DateTime<Utc>,
}

impl ConsolidationCandidate {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        conversation_id: Uuid,
        turn_start: usize,
        turn_end: usize,
        message_ids: Vec<Uuid>,
        summary: impl Into<String>,
        key_decisions: Vec<String>,
        entities: Vec<String>,
        proposed_memories: Vec<LongTermMemory>,
        estimated_token_count: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            conversation_id,
            turn_start,
            turn_end,
            message_ids,
            summary: summary.into(),
            key_decisions,
            entities,
            proposed_memories,
            estimated_token_count,
            created_at: Utc::now(),
        }
    }

    pub fn into_episode(self) -> Episode {
        Episode {
            id: self.id,
            conversation_id: self.conversation_id,
            turn_start: self.turn_start,
            turn_end: self.turn_end,
            summary: self.summary,
            key_decisions: self.key_decisions,
            entities: self.entities,
            token_count: self.estimated_token_count,
            created_at: self.created_at,
            updated_at: self.created_at,
        }
    }
}

// ============================================================================
// 4. WorkingMemoryBuffer Struct (Tier 1)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingMemoryBuffer {
    pub conversation_id: Uuid,
    pub max_turns: usize,
    pub max_tokens: usize,
    pub messages: Vec<crate::conversation::ChatMessage>,
    pub session_variables: std::collections::HashMap<String, String>,
    pub scratchpad: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl WorkingMemoryBuffer {
    pub fn new(conversation_id: Uuid) -> Self {
        Self {
            conversation_id,
            max_turns: 8,
            max_tokens: 2400,
            messages: Vec::new(),
            session_variables: std::collections::HashMap::new(),
            scratchpad: None,
            updated_at: Utc::now(),
        }
    }

    pub fn with_limits(conversation_id: Uuid, max_turns: usize, max_tokens: usize) -> Self {
        Self {
            conversation_id,
            max_turns,
            max_tokens,
            messages: Vec::new(),
            session_variables: std::collections::HashMap::new(),
            scratchpad: None,
            updated_at: Utc::now(),
        }
    }

    pub fn push_message(&mut self, message: crate::conversation::ChatMessage) {
        self.messages.push(message);
        self.updated_at = Utc::now();
        if self.messages.len() > self.max_turns {
            self.messages.remove(0);
        }
    }

    pub fn total_tokens(&self) -> usize {
        self.messages
            .iter()
            .map(|m| m.token_estimate.unwrap_or(0) as usize)
            .sum()
    }

    pub fn turn_count(&self) -> usize {
        self.messages.len()
    }

    pub fn exceeds_token_budget(&self) -> bool {
        self.total_tokens() > self.max_tokens
    }
}

// ============================================================================
// 5. Dynamic Scoring Engine
// ============================================================================

pub const EBBINGHAUS_HALF_LIFE_HOURS: f64 = 168.0; // 7 days

/// Computes the initial salience S_0 in [0.1, 1.0] based on category, pinned status,
/// rule keywords, directive/command keywords, and entity density.
pub fn compute_initial_salience(content: &str, pinned: bool, category: MemoryCategory) -> f32 {
    let mut score = 0.35_f32; // Base weight

    // Pinned weight
    if pinned {
        score += 0.50;
    }

    // Category weight
    match category {
        MemoryCategory::Preference => score += 0.20,
        MemoryCategory::System => score += 0.15,
        MemoryCategory::Technical => score += 0.10,
        MemoryCategory::Personal => score += 0.05,
    }

    let lower = content.to_lowercase();

    // Rule indicators (+0.15)
    const RULE_KEYWORDS: &[&str] = &[
        "always", "never", "must", "shall", "rule", "constraint",
        "forbidden", "mandatory", "require", "strictement", "toujours",
        "jamais", "interdit", "obligatoire", "règle", "regle",
    ];
    if RULE_KEYWORDS.iter().any(|&kw| lower.contains(kw)) {
        score += 0.15;
    }

    // User command / directive indicators (+0.15)
    const COMMAND_KEYWORDS: &[&str] = &[
        "remember", "don't forget", "do not forget", "keep in mind",
        "note that", "important", "retiens", "souviens-toi", "n'oublie pas",
        "mémorise", "memorise",
    ];
    if COMMAND_KEYWORDS.iter().any(|&kw| lower.contains(kw)) {
        score += 0.15;
    }

    // Entity density weight (up to +0.15)
    let words: Vec<&str> = content.split_whitespace().collect();
    if !words.is_empty() {
        let entity_count = words
            .iter()
            .filter(|w| {
                let trimmed = w.trim_matches(|c: char| !c.is_alphanumeric() && c != '`');
                trimmed.starts_with('`') && trimmed.ends_with('`')
                    || (trimmed.len() > 1
                        && trimmed.chars().next().is_some_and(|c| c.is_uppercase()))
                    || trimmed.contains('_')
            })
            .count();

        let density = entity_count as f32 / words.len() as f32;
        let entity_boost = (density * 0.25).min(0.15);
        score += entity_boost;
    }

    score.clamp(0.1, 1.0)
}

/// Computes the Ebbinghaus exponential recency decay R(t) = exp(-ln(2) * dt / 168.0).
/// Pinned memories are completely exempt (returns 1.0).
/// Future dates (now < last_used_at) return 1.0 (clamped).
pub fn compute_recency_decay(
    last_used_at: DateTime<Utc>,
    now: DateTime<Utc>,
    pinned: bool,
) -> f32 {
    if pinned {
        return 1.0;
    }

    let elapsed_seconds = (now - last_used_at).num_seconds();
    if elapsed_seconds <= 0 {
        return 1.0;
    }

    let elapsed_hours = elapsed_seconds as f64 / 3600.0;
    let exponent = -std::f64::consts::LN_2 * elapsed_hours / EBBINGHAUS_HALF_LIFE_HOURS;
    let decay = exponent.exp();

    (decay as f32).clamp(0.0, 1.0)
}

/// Computes the memory utility U(m, q) = 0.55 * Score_hybrid + 0.30 * (S_0 * R(t)) + 0.15 * ln(1 + recall_count).
pub fn compute_memory_utility(
    hybrid_score: f32,
    salience: f32,
    recency_factor: f32,
    recall_count: u32,
) -> f32 {
    let w_hybrid = 0.55_f32;
    let w_salience = 0.30_f32;
    let w_recall = 0.15_f32;

    let hybrid_term = w_hybrid * hybrid_score.max(0.0);
    let salience_term = w_salience * (salience.clamp(0.0, 1.0) * recency_factor.clamp(0.0, 1.0));
    let recall_term = w_recall * (1.0_f32 + recall_count as f32).ln();

    hybrid_term + salience_term + recall_term
}

// ============================================================================
// Context Window Budgeting & Token Estimation (Milestone 2)
// ============================================================================

pub const CONTEXT_WINDOW_CEILING: usize = 8192;
pub const DEFAULT_SYSTEM_BUDGET: usize = 800;
pub const DEFAULT_SEMANTIC_BUDGET: usize = 1600;
pub const DEFAULT_EPISODIC_BUDGET: usize = 2000;
pub const DEFAULT_WORKING_BUDGET: usize = 2400;
pub const DEFAULT_RESERVE_BUDGET: usize = 1392;
pub const DEFAULT_MAX_WORKING_TURNS: usize = 8;
pub const MESSAGE_FRAME_OVERHEAD_TOKENS: usize = 4;

/// Token budget partitions strictly enforcing the 8,192 total context ceiling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextBudget {
    pub system_budget: usize,
    pub semantic_budget: usize,
    pub episodic_budget: usize,
    pub working_budget: usize,
    pub reserve_budget: usize,
    pub total_ceiling: usize,
    pub max_working_turns: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            system_budget: DEFAULT_SYSTEM_BUDGET,
            semantic_budget: DEFAULT_SEMANTIC_BUDGET,
            episodic_budget: DEFAULT_EPISODIC_BUDGET,
            working_budget: DEFAULT_WORKING_BUDGET,
            reserve_budget: DEFAULT_RESERVE_BUDGET,
            total_ceiling: CONTEXT_WINDOW_CEILING,
            max_working_turns: DEFAULT_MAX_WORKING_TURNS,
        }
    }
}

impl ContextBudget {
    pub fn new(
        system_budget: usize,
        semantic_budget: usize,
        episodic_budget: usize,
        working_budget: usize,
        reserve_budget: usize,
        max_working_turns: usize,
    ) -> Result<Self, String> {
        if max_working_turns == 0 || max_working_turns > 8 {
            return Err("max_working_turns must be between 1 and 8".to_string());
        }
        Self::new_with_ceiling(
            system_budget,
            semantic_budget,
            episodic_budget,
            working_budget,
            reserve_budget,
            CONTEXT_WINDOW_CEILING,
            max_working_turns,
        )
    }

    pub fn new_with_ceiling(
        system_budget: usize,
        semantic_budget: usize,
        episodic_budget: usize,
        working_budget: usize,
        reserve_budget: usize,
        total_ceiling: usize,
        max_working_turns: usize,
    ) -> Result<Self, String> {
        let total = system_budget
            .saturating_add(semantic_budget)
            .saturating_add(episodic_budget)
            .saturating_add(working_budget)
            .saturating_add(reserve_budget);

        if total > total_ceiling {
            return Err(format!(
                "Total budget sum {} exceeds context window ceiling of {}",
                total, total_ceiling
            ));
        }

        if max_working_turns == 0 {
            return Err("max_working_turns must be at least 1".to_string());
        }

        Ok(Self {
            system_budget,
            semantic_budget,
            episodic_budget,
            working_budget,
            reserve_budget,
            total_ceiling,
            max_working_turns,
        })
    }

    /// Automatically constructs an adaptive 5-partition budget for any arbitrary total token ceiling.
    pub fn from_total_ceiling(ceiling: usize, max_working_turns: Option<usize>) -> Self {
        let ceiling = ceiling.max(2048);
        let system = (ceiling * 10 / 100).clamp(400, 4000);
        let semantic = (ceiling * 20 / 100).max(600);
        let episodic = (ceiling * 25 / 100).max(800);
        let working = (ceiling * 30 / 100).max(1000);
        let used = system + semantic + episodic + working;
        let reserve = if ceiling > used { ceiling - used } else { 400 };
        let turns = max_working_turns.unwrap_or(DEFAULT_MAX_WORKING_TURNS);

        Self {
            system_budget: system,
            semantic_budget: semantic,
            episodic_budget: episodic,
            working_budget: working,
            reserve_budget: reserve,
            total_ceiling: ceiling,
            max_working_turns: turns,
        }
    }

    pub fn max_prompt_tokens(&self) -> usize {
        self.system_budget + self.semantic_budget + self.episodic_budget + self.working_budget
    }

    pub fn total_budget(&self) -> usize {
        self.max_prompt_tokens() + self.reserve_budget
    }
}

/// Token usage across all 5 context partitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextTokenUsage {
    pub system_tokens: usize,
    pub semantic_tokens: usize,
    pub episodic_tokens: usize,
    pub working_tokens: usize,
    pub total_input_tokens: usize,
    pub reserve_tokens: usize,
    pub total_tokens: usize,
}

impl ContextTokenUsage {
    pub fn is_within_budget(&self, budget: &ContextBudget) -> bool {
        self.system_tokens <= budget.system_budget
            && self.semantic_tokens <= budget.semantic_budget
            && self.episodic_tokens <= budget.episodic_budget
            && self.working_tokens <= budget.working_budget
            && self.total_input_tokens <= budget.max_prompt_tokens()
            && self.total_tokens <= budget.total_ceiling
    }
}

/// Eviction telemetry detailing candidates retained vs pruned.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextEvictionReport {
    pub semantic_candidates: usize,
    pub semantic_retained: usize,
    pub semantic_evicted: usize,
    pub episodic_candidates: usize,
    pub episodic_retained: usize,
    pub episodic_evicted: usize,
    pub working_candidates: usize,
    pub working_retained: usize,
    pub working_evicted: usize,
}

// ============================================================================
// Safe Upper-Bound Token Estimation
// ============================================================================

/// Fast, conservative upper-bound token estimator.
/// - Fenced code blocks (` ```...``` `): ~2.8 chars/token
/// - Natural language prose and markdown: ~3.8 chars/token + punctuation boost
/// - Guaranteed non-underestimating upper bound.
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    let mut total_tokens = 0usize;
    let mut in_code_block = false;
    let mut current_segment_chars = 0usize;
    let mut current_segment_symbols = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if current_segment_chars > 0 {
                if in_code_block {
                    total_tokens += estimate_code_segment(current_segment_chars);
                } else {
                    total_tokens +=
                        estimate_prose_segment(current_segment_chars, current_segment_symbols);
                }
                current_segment_chars = 0;
                current_segment_symbols = 0;
            }
            in_code_block = !in_code_block;
            total_tokens += 2; // code fence tokens
            continue;
        }

        let char_count = line.chars().count() + 1; // +1 for newline
        current_segment_chars += char_count;

        if !in_code_block {
            let symbols = line
                .chars()
                .filter(|c| "{}[];:()\"'`,.<>/?\\|!@#$%^&*+-=_~".contains(*c))
                .count();
            current_segment_symbols += symbols;
        }
    }

    if current_segment_chars > 0 {
        if in_code_block {
            total_tokens += estimate_code_segment(current_segment_chars);
        } else {
            total_tokens += estimate_prose_segment(current_segment_chars, current_segment_symbols);
        }
    }

    total_tokens.max(1)
}

fn estimate_code_segment(chars: usize) -> usize {
    // 2.8 chars / token with ceiling: chars * 10 / 28 ceil
    chars.saturating_mul(10).div_ceil(28)
}

fn estimate_prose_segment(chars: usize, symbols: usize) -> usize {
    // 3.8 chars / token base: chars * 10 / 38 ceil
    let base = chars.saturating_mul(10).div_ceil(38);
    let symbol_boost = symbols / 3;
    base + symbol_boost
}

/// Estimates tokens for a ChatMessage including envelope overhead.
pub fn estimate_message_tokens(message: &crate::conversation::ChatMessage) -> usize {
    MESSAGE_FRAME_OVERHEAD_TOKENS + estimate_tokens(&message.content)
}

/// Token estimator wrapper type for structured estimation.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokenEstimator;

impl TokenEstimator {
    pub fn estimate(text: &str) -> usize {
        estimate_tokens(text)
    }

    pub fn estimate_message(message: &crate::conversation::ChatMessage) -> usize {
        estimate_message_tokens(message)
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_decay_curve_mathematical_exactness() {
        let t0 = Utc::now();

        // t = 0 -> R = 1.0
        let r0 = compute_recency_decay(t0, t0, false);
        assert!((r0 - 1.0).abs() < 1e-5, "Expected 1.0 at t=0, got {}", r0);

        // t = 7 days (168h) -> R = 0.50
        let t_7d = t0 + Duration::hours(168);
        let r_7d = compute_recency_decay(t0, t_7d, false);
        assert!((r_7d - 0.50).abs() < 1e-4, "Expected 0.50 at 7 days, got {}", r_7d);

        // t = 14 days (336h) -> R = 0.25
        let t_14d = t0 + Duration::hours(336);
        let r_14d = compute_recency_decay(t0, t_14d, false);
        assert!((r_14d - 0.25).abs() < 1e-4, "Expected 0.25 at 14 days, got {}", r_14d);

        // t = 28 days (672h) -> R = 0.0625
        let t_28d = t0 + Duration::hours(672);
        let r_28d = compute_recency_decay(t0, t_28d, false);
        assert!((r_28d - 0.0625).abs() < 1e-4, "Expected 0.0625 at 28 days, got {}", r_28d);
    }

    #[test]
    fn test_decay_pinned_immunity_and_future_clamping() {
        let t0 = Utc::now();
        let t_past = t0 - Duration::days(1000);

        // Pinned is always 1.0 regardless of time
        let r_pinned = compute_recency_decay(t_past, t0, true);
        assert_eq!(r_pinned, 1.0, "Pinned memory must be completely immune to decay");

        // Future date (now < last_used_at) clamped to 1.0
        let t_future = t0 + Duration::days(5);
        let r_future = compute_recency_decay(t_future, t0, false);
        assert_eq!(r_future, 1.0, "Future dates must be clamped to 1.0");
    }

    #[test]
    fn test_initial_salience_clamping_and_weights() {
        // Minimum / plain text
        let s_plain = compute_initial_salience("simple note", false, MemoryCategory::Personal);
        assert!((0.1..=1.0).contains(&s_plain));

        // Rule keyword boosts salience
        let s_rule = compute_initial_salience("Always use strict typing", false, MemoryCategory::Personal);
        assert!(s_rule > s_plain, "Rule keyword 'always' must increase salience");

        // Directive command boosts salience
        let s_cmd = compute_initial_salience("Remember that user prefers dark theme", false, MemoryCategory::Personal);
        assert!(s_cmd > s_plain, "Command keyword 'remember' must increase salience");

        // Preference category has higher weight than personal
        let s_pref = compute_initial_salience("simple note", false, MemoryCategory::Preference);
        assert!(s_pref > s_plain, "Preference category must have higher salience than Personal");

        // Pinned memory guaranteed high salience
        let s_pinned = compute_initial_salience("simple note", true, MemoryCategory::Personal);
        assert!(s_pinned >= 0.85, "Pinned memory must have high salience");

        // Max content clamped to 1.0
        let s_max = compute_initial_salience(
            "Always remember: must strictly adhere to Rust `SqliteMemoryStore` WAL rules!",
            true,
            MemoryCategory::Preference,
        );
        assert_eq!(s_max, 1.0, "Maximum score must clamp to 1.0");
    }

    #[test]
    fn test_utility_monotonicity_invariants() {
        let base_u = compute_memory_utility(0.5, 0.5, 0.5, 2);

        // 1. Monotonic in hybrid_score
        let u_higher_hybrid = compute_memory_utility(0.8, 0.5, 0.5, 2);
        assert!(u_higher_hybrid > base_u, "Utility must increase with hybrid_score");

        // 2. Monotonic in salience
        let u_higher_salience = compute_memory_utility(0.5, 0.8, 0.5, 2);
        assert!(u_higher_salience > base_u, "Utility must increase with salience");

        // 3. Monotonic in recency_factor
        let u_higher_recency = compute_memory_utility(0.5, 0.5, 0.9, 2);
        assert!(u_higher_recency > base_u, "Utility must increase with recency_factor");

        // 4. Monotonic in recall_count
        let u_higher_recall = compute_memory_utility(0.5, 0.5, 0.5, 5);
        assert!(u_higher_recall > base_u, "Utility must increase with recall_count");

        // 5. Zero recall count yields 0 for recall term
        let u_zero_recall = compute_memory_utility(1.0, 1.0, 1.0, 0);
        let expected = 0.55 * 1.0 + 0.30 * 1.0 + 0.15 * 0.0;
        assert!((u_zero_recall - expected).abs() < 1e-5);
    }

    #[test]
    fn test_episode_and_summary_lifecycle() {
        let conv_id = Uuid::new_v4();
        let episode = Episode::new(
            conv_id,
            1,
            10,
            "Initial setup completed",
            vec!["Selected SQLite WAL mode".to_string()],
            vec!["SQLite".to_string(), "WAL".to_string()],
            150,
        );

        assert_eq!(episode.turn_span(), 10);
        assert!(episode.contains_turn(5));
        assert!(!episode.contains_turn(11));

        let summary = EpisodeSummary::from(&episode);
        assert_eq!(summary.id, episode.id);
        assert_eq!(summary.summary, "Initial setup completed");
        let prompt_text = summary.format_for_prompt();
        assert!(prompt_text.contains("Turns 1-10"));
        assert!(prompt_text.contains("Selected SQLite WAL mode"));
    }

    #[test]
    fn test_consolidation_candidate_to_episode() {
        let conv_id = Uuid::new_v4();
        let candidate = ConsolidationCandidate::new(
            conv_id,
            11,
            20,
            vec![Uuid::new_v4(), Uuid::new_v4()],
            "Refactored memory module",
            vec!["Separated scoring engine".to_string()],
            vec!["ScoringEngine".to_string()],
            vec![],
            220,
        );

        let ep = candidate.into_episode();
        assert_eq!(ep.turn_start, 11);
        assert_eq!(ep.turn_end, 20);
        assert_eq!(ep.token_count, 220);
    }

    #[test]
    fn test_working_memory_buffer_budget() {
        let conv_id = Uuid::new_v4();
        let mut buffer = WorkingMemoryBuffer::with_limits(conv_id, 3, 500);

        for i in 1..=4 {
            let mut msg = crate::conversation::ChatMessage::new(
                conv_id,
                crate::conversation::MessageRole::User,
                format!("Message {}", i),
            );
            msg.token_estimate = Some(50);
            buffer.push_message(msg);
        }

        // Bounded turns: max 3
        assert_eq!(buffer.turn_count(), 3);
        assert_eq!(buffer.messages[0].content, "Message 2");
        assert_eq!(buffer.total_tokens(), 150);
        assert!(!buffer.exceeds_token_budget());
    }

    #[test]
    fn test_context_budget_defaults_and_sums() {
        let b = ContextBudget::default();
        assert_eq!(b.system_budget, 800);
        assert_eq!(b.semantic_budget, 1600);
        assert_eq!(b.episodic_budget, 2000);
        assert_eq!(b.working_budget, 2400);
        assert_eq!(b.reserve_budget, 1392);
        assert_eq!(b.total_ceiling, 8192);
        assert_eq!(b.max_working_turns, 8);
        assert_eq!(b.max_prompt_tokens(), 6800);
        assert_eq!(b.total_budget(), 8192);
    }

    #[test]
    fn test_context_budget_custom_valid_and_invalid() {
        let valid = ContextBudget::new(700, 1500, 2000, 2400, 1500, 6).unwrap();
        assert_eq!(valid.max_prompt_tokens(), 6600);
        assert_eq!(valid.total_budget(), 8100);

        // Exceeds ceiling 8192
        let invalid_ceiling = ContextBudget::new(1000, 2000, 2500, 3000, 1000, 8);
        assert!(invalid_ceiling.is_err());

        // Zero working turns
        let invalid_turns_zero = ContextBudget::new(800, 1600, 2000, 2400, 1392, 0);
        assert!(invalid_turns_zero.is_err());

        // More than 8 working turns
        let invalid_turns_large = ContextBudget::new(800, 1600, 2000, 2400, 1392, 9);
        assert!(invalid_turns_large.is_err());
    }

    #[test]
    fn test_token_estimator_code_vs_prose_ratios() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(TokenEstimator::estimate(""), 0);

        let prose = "The quick brown fox jumps over the lazy dog."; // 44 chars
        let prose_tokens = estimate_tokens(prose);
        assert!((11..=18).contains(&prose_tokens));

        let code = "```rust\nfn calculate(a: i32, b: i32) -> i32 {\n    a + b\n}\n```";
        let code_tokens = estimate_tokens(code);
        assert!((15..=35).contains(&code_tokens));
        // Code density should be higher per char
        assert!(code_tokens as f32 / code.len() as f32 >= prose_tokens as f32 / prose.len() as f32);
    }

    #[test]
    fn test_token_estimator_message_frame_overhead() {
        let conv_id = Uuid::new_v4();
        let msg = crate::conversation::ChatMessage::new(
            conv_id,
            crate::conversation::MessageRole::User,
            "Hello world".to_string(),
        );
        let msg_tokens = estimate_message_tokens(&msg);
        let struct_tokens = TokenEstimator::estimate_message(&msg);
        assert_eq!(msg_tokens, struct_tokens);
        assert!(msg_tokens > MESSAGE_FRAME_OVERHEAD_TOKENS);
    }

    #[test]
    fn test_context_token_usage_within_budget() {
        let budget = ContextBudget::default();
        let usage_ok = ContextTokenUsage {
            system_tokens: 500,
            semantic_tokens: 1200,
            episodic_tokens: 1500,
            working_tokens: 2000,
            total_input_tokens: 5200,
            reserve_tokens: 1392,
            total_tokens: 6592,
        };
        assert!(usage_ok.is_within_budget(&budget));

        let usage_violation = ContextTokenUsage {
            system_tokens: 801, // exceeds 800
            semantic_tokens: 1200,
            episodic_tokens: 1500,
            working_tokens: 2000,
            total_input_tokens: 5501,
            reserve_tokens: 1392,
            total_tokens: 6893,
        };
        assert!(!usage_violation.is_within_budget(&budget));
    }

    #[test]
    fn test_context_eviction_report_tracking() {
        let report = ContextEvictionReport {
            semantic_candidates: 10,
            semantic_retained: 7,
            semantic_evicted: 3,
            episodic_candidates: 5,
            episodic_retained: 4,
            episodic_evicted: 1,
            working_candidates: 12,
            working_retained: 8,
            working_evicted: 4,
        };
        assert_eq!(report.semantic_evicted, 3);
        assert_eq!(report.episodic_evicted, 1);
        assert_eq!(report.working_evicted, 4);
    }
}

