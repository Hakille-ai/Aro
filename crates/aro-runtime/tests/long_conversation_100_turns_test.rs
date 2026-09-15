//! Milestone 4 Integration Test Suite: 100+ Turn Long-Conversation Benchmark,
//! Token Ceiling Enforcement ($B \le 8,192$), Multi-Layer Recall & Cold Restart Resilience
//!
//! Strict Compliance:
//! - ORIGINAL_REQUEST.md §AC1: 100+ turn conversation with Fact A (turn 5) and Fact B (turn 20)
//!   recalled accurately at turn 95+.
//! - ORIGINAL_REQUEST.md §AC1: Token volume injected into context never exceeds 8,192 tokens.
//! - ORIGINAL_REQUEST.md §AC2: 100% test pass, thread-safe, transactional, restart resilient.
//! - ORIGINAL_REQUEST.md §AC3: Agent memory tools (memory_search, memory_recall) return facts
//!   and associated Episode 1 & 2 records.
//! - Cold restart resilience at Turn 101 recovers last_compacted_turn = 100 and intact episodes.
#![allow(clippy::manual_is_multiple_of)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use aro_core::{
    estimate_message_tokens, estimate_tokens, AgentRun, AssistantMode, ChatMessage, ContextBudget,
    Conversation, EpisodeSummary, MessageRole, ModelGeneration, ModelGenerationRequest,
    ModelProviderKind, ModelResponseFormat, RuntimeStatus, SendMessageRequest,
    ToolExecutionRequest, ToolExecutionStatus, WebAccessMode, WorkingMemoryBuffer, TOOL_CORE_MEMORY_RECALL,
    TOOL_CORE_MEMORY_SEARCH, TOOL_MEMORY_RECALL, TOOL_MEMORY_SEARCH,
};
use aro_memory::SqliteMemoryStore;
use aro_runtime::context_manager::{
    AssembledContext, CompactionState, ContinuousCompactor, ContextWindowManager,
};
use aro_runtime::{AssistantEngine, ModelProvider};
use aro_vector::{MemoryVectorService, VectorMemoryConfig, VectorMemoryMode};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

// ============================================================================
// 1. LongConversationMockProvider
// ============================================================================

/// Genuine Mock Model Provider that inspects the assembled context and system prompt
/// passed to it. It only recalls facts when they are present in the cognitive memory context.
#[derive(Clone)]
pub struct LongConversationMockProvider {
    pub call_count: Arc<AtomicUsize>,
}

impl LongConversationMockProvider {
    pub fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Default for LongConversationMockProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ModelProvider for LongConversationMockProvider {
    async fn generate(&self, request: ModelGenerationRequest) -> aro_core::AroResult<ModelGeneration> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let user_input_lower = request.user_input.to_lowercase();
        let sys_prompt = &request.system_prompt;

        // Verify Fact A presence in cognitive memory context
        let is_fact_a_query = user_input_lower.contains("fact a")
            || (user_input_lower.contains("prometheus") && user_input_lower.contains("port"))
            || (user_input_lower.contains("telemetry") && user_input_lower.contains("port"));

        // Verify Fact B presence in cognitive memory context
        let is_fact_b_query = user_input_lower.contains("fact b")
            || user_input_lower.contains("infra-stg-7712")
            || (user_input_lower.contains("secret") && user_input_lower.contains("cluster"))
            || (user_input_lower.contains("deployment") && user_input_lower.contains("secret"));

        // Verify joint query
        let is_joint_query = (user_input_lower.contains("both")
            || user_input_lower.contains("reboot")
            || user_input_lower.contains("restart")
            || user_input_lower.contains("all configurations"))
            && (is_fact_a_query || is_fact_b_query || user_input_lower.contains("telemetry"));

        let content = if is_joint_query {
            let has_a = sys_prompt.contains("9464") && sys_prompt.contains("/aro-metrics");
            let has_b = sys_prompt.contains("infra-stg-7712") && sys_prompt.contains("k8s-eu-west-3");
            if has_a && has_b {
                "Recalled both configurations: Prometheus port 9464, route /aro-metrics, and secret ID infra-stg-7712, cluster k8s-eu-west-3.".to_string()
            } else if has_a {
                "Recalled Prometheus port 9464, route /aro-metrics (Fact B missing in context).".to_string()
            } else if has_b {
                "Recalled secret ID infra-stg-7712, cluster k8s-eu-west-3 (Fact A missing in context).".to_string()
            } else {
                "Neither Fact A nor Fact B found in assembled cognitive context.".to_string()
            }
        } else if is_fact_a_query {
            let has_port = sys_prompt.contains("9464");
            let has_route = sys_prompt.contains("/aro-metrics");
            if has_port && has_route {
                "Recalled from cognitive memory: Prometheus port 9464, route /aro-metrics.".to_string()
            } else {
                "Fact A not found in assembled context.".to_string()
            }
        } else if is_fact_b_query {
            let has_secret = sys_prompt.contains("infra-stg-7712");
            let has_cluster = sys_prompt.contains("k8s-eu-west-3");
            if has_secret && has_cluster {
                "Recalled from cognitive memory: secret ID infra-stg-7712, cluster k8s-eu-west-3.".to_string()
            } else {
                "Fact B not found in assembled context.".to_string()
            }
        } else {
            // Standard conversational turn confirmation
            format!("Confirmed turn processing for: {}", request.user_input)
        };

        // Format according to requested model response format
        let wants_json = request.response_format == ModelResponseFormat::AgentActionJson
            || request.system_prompt.contains("Return exactly one JSON object");
        let payload = if wants_json {
            json!({
                "type": "final",
                "content": content
            })
            .to_string()
        } else {
            content
        };

        Ok(ModelGeneration {
            content: payload,
            provider_detail: "LongConversationMockProvider".to_string(),
            token_estimate: Some(64),
        })
    }

    async fn generate_stream(
        &self,
        request: ModelGenerationRequest,
        _on_chunk: &mut (dyn FnMut(String) + Send),
    ) -> aro_core::AroResult<ModelGeneration> {
        self.generate(request).await
    }

    async fn status(&self) -> RuntimeStatus {
        RuntimeStatus {
            model_provider: ModelProviderKind::Mock,
            model_id: "mock-long-conv".to_string(),
            model_ready: true,
            voice_ready: false,
            endpoint: None,
            detail: "Long Conversation Mock Provider Ready".to_string(),
            checked_at: Utc::now(),
        }
    }

    fn temperature(&self) -> f32 {
        0.0
    }

    fn max_tokens(&self) -> u32 {
        1024
    }
}

// ============================================================================
// 2. Test Harness Helpers
// ============================================================================

/// Creates an isolated temporary SQLite store on disk and an AssistantEngine
/// with vector memory configured for local in-memory/FTS operation.
fn create_test_harness() -> (AssistantEngine, SqliteMemoryStore, PathBuf) {
    // Disable external Qdrant network calls to ensure deterministic, zero-latency execution
    std::env::set_var("ARO_VECTOR_MEMORY_MODE", "disabled");

    let db_path = std::env::temp_dir().join(format!(
        "aro-m4-100turn-bench-{}.sqlite",
        Uuid::new_v4()
    ));
    let store = SqliteMemoryStore::new(&db_path).expect("failed to create sqlite memory store");

    let mut vector_cfg = VectorMemoryConfig::from_env();
    vector_cfg.mode = VectorMemoryMode::Disabled;
    let vector_service = MemoryVectorService::new(vector_cfg);

    let engine = AssistantEngine::with_vector_service(store.clone(), vector_service);
    (engine, store, db_path)
}

fn cleanup_temp_db(path: PathBuf) {
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

fn create_test_agent_run(store: &SqliteMemoryStore, conv_id: Uuid) -> AgentRun {
    let run = AgentRun::new(
        "M4 Agent Run",
        AssistantMode::Chat,
        Some(conv_id),
        None,
        None,
        None,
    );
    store.upsert_agent_run(&run).expect("upsert agent run");
    run
}

// ============================================================================
// 3. Invariant Assertion Helpers
// ============================================================================

/// Asserts strict compliance with the 8,192 token context window ceiling
/// and all individual partition limits for a given conversational turn.
fn assert_context_window_ceiling_invariants(
    turn: usize,
    assembled: &AssembledContext,
    budget: &ContextBudget,
) {
    let usage = &assembled.token_usage;

    // 1. Overall Ceiling: B_total <= 8,192
    assert!(
        usage.total_tokens <= 8192,
        "Turn {}: Total assembled tokens {} strictly EXCEEDS 8,192 ceiling!",
        turn,
        usage.total_tokens
    );
    assert!(
        usage.total_tokens <= budget.total_ceiling,
        "Turn {}: Total tokens {} exceeds budget ceiling {}",
        turn,
        usage.total_tokens,
        budget.total_ceiling
    );

    // 2. Partition 1: System prompt instructions <= 800 tokens
    assert!(
        usage.system_tokens <= 800,
        "Turn {}: System prompt tokens {} exceeds 800 budget limit",
        turn,
        usage.system_tokens
    );
    assert!(
        usage.system_tokens <= budget.system_budget,
        "Turn {}: System tokens {} exceeds configured budget {}",
        turn,
        usage.system_tokens,
        budget.system_budget
    );

    // 3. Partition 2: Long-term semantic knowledge <= 1,600 tokens
    assert!(
        usage.semantic_tokens <= 1600,
        "Turn {}: Semantic tokens {} exceeds 1,600 budget limit",
        turn,
        usage.semantic_tokens
    );
    assert!(
        usage.semantic_tokens <= budget.semantic_budget,
        "Turn {}: Semantic tokens {} exceeds configured budget {}",
        turn,
        usage.semantic_tokens,
        budget.semantic_budget
    );

    // 4. Partition 3: Chronological episodic history <= 2,000 tokens
    assert!(
        usage.episodic_tokens <= 2000,
        "Turn {}: Episodic tokens {} exceeds 2,000 budget limit",
        turn,
        usage.episodic_tokens
    );
    assert!(
        usage.episodic_tokens <= budget.episodic_budget,
        "Turn {}: Episodic tokens {} exceeds configured budget {}",
        turn,
        usage.episodic_tokens,
        budget.episodic_budget
    );

    // 5. Partition 4: Working memory buffer <= 2,400 tokens
    assert!(
        usage.working_tokens <= 2400,
        "Turn {}: Working memory tokens {} exceeds 2,400 budget limit",
        turn,
        usage.working_tokens
    );
    assert!(
        usage.working_tokens <= budget.working_budget,
        "Turn {}: Working tokens {} exceeds configured budget {}",
        turn,
        usage.working_tokens,
        budget.working_budget
    );

    // 6. Partition 5: Completion headroom reserve = 1,392 tokens
    assert_eq!(
        usage.reserve_tokens, 1392,
        "Turn {}: Reserve completion headroom must be exactly 1,392 tokens, got {}",
        turn, usage.reserve_tokens
    );
    assert_eq!(
        usage.reserve_tokens, budget.reserve_budget,
        "Turn {}: Reserve tokens mismatch with budget",
        turn
    );

    // 7. Input tokens summation check: sys + sem + epi + work <= 6,800
    let calculated_input = usage.system_tokens
        + usage.semantic_tokens
        + usage.episodic_tokens
        + usage.working_tokens;
    assert_eq!(
        usage.total_input_tokens, calculated_input,
        "Turn {}: total_input_tokens {} does not match partition sum {}",
        turn, usage.total_input_tokens, calculated_input
    );
    assert!(
        usage.total_input_tokens <= budget.max_prompt_tokens(),
        "Turn {}: Total input tokens {} exceeds max prompt tokens {}",
        turn,
        usage.total_input_tokens,
        budget.max_prompt_tokens()
    );

    // 8. Total tokens summation check: total_input + reserve = total
    assert_eq!(
        usage.total_tokens,
        usage.total_input_tokens + usage.reserve_tokens,
        "Turn {}: total_tokens does not equal total_input_tokens + reserve_tokens",
        turn
    );

    // 9. Struct helper validator
    assert!(
        usage.is_within_budget(budget),
        "Turn {}: ContextTokenUsage::is_within_budget returned false",
        turn
    );
}

/// Asserts working memory buffer bounds: N <= 8 messages and <= 2,400 tokens,
/// reverse-recency preservation, and eviction telemetry accuracy.
fn assert_working_memory_buffer_invariants(
    turn: usize,
    assembled: &AssembledContext,
    total_messages_in_history: usize,
    budget: &ContextBudget,
) {
    let working = &assembled.working_messages;
    let eviction = &assembled.eviction_report;

    // 1. Strict message count bound: N <= 8
    assert!(
        working.len() <= 8,
        "Turn {}: Working memory messages count {} EXCEEDS max 8 limit!",
        turn,
        working.len()
    );
    assert!(
        working.len() <= budget.max_working_turns,
        "Turn {}: Working messages count {} exceeds budget max_working_turns {}",
        turn,
        working.len(),
        budget.max_working_turns
    );

    // If total messages in conversation >= 8, buffer should be full (8 messages)
    if total_messages_in_history >= 8 {
        assert_eq!(
            working.len(),
            8,
            "Turn {}: When history has {} messages (>= 8), working buffer must contain exactly 8 messages",
            turn,
            total_messages_in_history
        );
    } else {
        assert_eq!(
            working.len(),
            total_messages_in_history,
            "Turn {}: When history has {} messages (< 8), working buffer must contain all messages",
            turn,
            total_messages_in_history
        );
    }

    // 2. Strict working token ceiling: <= 2,400 tokens
    let computed_tokens: usize = working.iter().map(estimate_message_tokens).sum();
    assert_eq!(
        assembled.token_usage.working_tokens, computed_tokens,
        "Turn {}: Token usage working_tokens {} does not match computed token sum {}",
        turn, assembled.token_usage.working_tokens, computed_tokens
    );
    assert!(
        computed_tokens <= 2400,
        "Turn {}: Working messages token count {} EXCEEDS 2,400 budget limit",
        turn,
        computed_tokens
    );

    // 3. Eviction report verification
    assert_eq!(
        eviction.working_candidates, total_messages_in_history,
        "Turn {}: working_candidates {} does not match total history count {}",
        turn, eviction.working_candidates, total_messages_in_history
    );
    assert_eq!(
        eviction.working_retained,
        working.len(),
        "Turn {}: working_retained {} does not match working_messages.len() {}",
        turn,
        eviction.working_retained,
        working.len()
    );
    assert_eq!(
        eviction.working_evicted,
        total_messages_in_history.saturating_sub(working.len()),
        "Turn {}: working_evicted {} does not match candidates - retained",
        turn,
        eviction.working_evicted
    );
}

/// Asserts that continuous compaction triggers every 10 turns (turns 10, 20, 30, ..., 100).
fn assert_compaction_cadence_invariant(
    turn: usize,
    store: &SqliteMemoryStore,
    conv_id: Uuid,
    compactor: &ContinuousCompactor,
) {
    let expected_episodes_count = turn / 10;
    let stored_episodes = store
        .list_episodes(conv_id)
        .expect("must list episodes from sqlite");

    assert_eq!(
        stored_episodes.len(),
        expected_episodes_count,
        "Turn {}: Expected exactly {} episodes stored in SQLite, found {}",
        turn,
        expected_episodes_count,
        stored_episodes.len()
    );

    if turn % 10 == 0 {
        let latest = store
            .get_latest_episode(conv_id)
            .expect("must get latest episode")
            .expect("latest episode must exist at multiple of 10 turn");
        let expected_turn_start = turn - 9;
        let expected_turn_end = turn;

        assert_eq!(
            latest.turn_start, expected_turn_start,
            "Turn {}: Latest episode turn_start expected {}, got {}",
            turn, expected_turn_start, latest.turn_start
        );
        assert_eq!(
            latest.turn_end, expected_turn_end,
            "Turn {}: Latest episode turn_end expected {}, got {}",
            turn, expected_turn_end, latest.turn_end
        );

        // Idempotency check: immediate second consolidation call returns None
        let second_call = compactor
            .consolidate_if_needed(conv_id)
            .expect("second consolidation check");
        assert!(
            second_call.is_none(),
            "Turn {}: ContinuousCompactor must be idempotent",
            turn
        );
    }
}

/// Asserts that all 10 `Episode` records are created and saved in SQLite with
/// contiguous non-overlapping turn ranges, valid entity summaries, decisions,
/// positive token compression ratios, and FTS5 searchability.
fn assert_all_10_episodes_integrity(
    store: &SqliteMemoryStore,
    conv_id: Uuid,
    fact_a_marker: &str, // "9464"
    fact_b_marker: &str, // "infra-stg-7712"
) {
    let episodes = store
        .list_episodes(conv_id)
        .expect("must list episodes from sqlite");

    // 1. Total Episode count must be exactly 10
    assert_eq!(
        episodes.len(),
        10,
        "Expected exactly 10 consolidated episodes for 100 turns, found {}",
        episodes.len()
    );

    // 2. Validate contiguous, non-overlapping turn ranges across all 10 episodes
    for (k, ep) in episodes.iter().enumerate() {
        let expected_start = k * 10 + 1;
        let expected_end = (k + 1) * 10;

        assert_eq!(
            ep.turn_start,
            expected_start,
            "Episode {} turn_start expected {}, got {}",
            k + 1,
            expected_start,
            ep.turn_start
        );
        assert_eq!(
            ep.turn_end,
            expected_end,
            "Episode {} turn_end expected {}, got {}",
            k + 1,
            expected_end,
            ep.turn_end
        );
        assert_eq!(
            ep.turn_span(),
            10,
            "Episode {} turn_span must be exactly 10",
            k + 1
        );
        assert_eq!(
            ep.conversation_id, conv_id,
            "Episode {} conversation_id mismatch",
            k + 1
        );

        // Strict contiguous chaining invariant: ep[k].turn_start == ep[k-1].turn_end + 1
        if k > 0 {
            assert_eq!(
                ep.turn_start,
                episodes[k - 1].turn_end + 1,
                "Episode {} (turn_start {}) is not contiguous with Episode {} (turn_end {})",
                k + 1,
                ep.turn_start,
                k,
                episodes[k - 1].turn_end
            );
        }

        // 3. Validate Summary narrative structure
        assert!(
            !ep.summary.trim().is_empty(),
            "Episode {} summary must not be empty",
            k + 1
        );
        let expected_header = format!("Consolidated turns {}-{}", expected_start, expected_end);
        assert!(
            ep.summary.contains(&expected_header),
            "Episode {} summary missing header '{}'. Summary: {}",
            k + 1,
            expected_header,
            ep.summary
        );

        // 4. Validate Extracted Entities or Key Decisions
        assert!(
            !ep.entities.is_empty() || !ep.key_decisions.is_empty(),
            "Episode {} must contain extracted entities or decisions. Summary: {}",
            k + 1,
            ep.summary
        );

        // 5. Validate Token Compression
        assert!(
            ep.token_count > 0,
            "Episode {} token_count must be positive",
            k + 1
        );
        let formatted = EpisodeSummary::from(ep).format_for_prompt();
        let formatted_tokens = estimate_tokens(&formatted);
        assert!(
            formatted_tokens < ep.token_count,
            "Episode {} summary tokens ({}) must be strictly less than uncompacted message tokens ({})",
            k + 1,
            formatted_tokens,
            ep.token_count
        );
    }

    // 6. Fact A verification in Episode 1 (turns 1-10 containing turn 5)
    let ep1 = &episodes[0];
    let ep1_has_fact_a = ep1.entities.iter().any(|e| e.contains(fact_a_marker))
        || ep1.key_decisions.iter().any(|d| d.contains(fact_a_marker))
        || ep1.summary.contains(fact_a_marker);
    assert!(
        ep1_has_fact_a,
        "Episode 1 (turns 1-10) MUST retain Fact A marker '{}'. Entities: {:?}, Decisions: {:?}, Summary: {}",
        fact_a_marker, ep1.entities, ep1.key_decisions, ep1.summary
    );

    // 7. Fact B verification in Episode 2 (turns 11-20 containing turn 20)
    let ep2 = &episodes[1];
    let ep2_has_fact_b = ep2.entities.iter().any(|e| e.contains(fact_b_marker))
        || ep2.key_decisions.iter().any(|d| d.contains(fact_b_marker))
        || ep2.summary.contains(fact_b_marker);
    assert!(
        ep2_has_fact_b,
        "Episode 2 (turns 11-20) MUST retain Fact B marker '{}'. Entities: {:?}, Decisions: {:?}, Summary: {}",
        fact_b_marker, ep2.entities, ep2.key_decisions, ep2.summary
    );

    // 8. Latest episode query verification
    let latest = store
        .get_latest_episode(conv_id)
        .expect("must get latest episode")
        .expect("latest episode must exist");
    assert_eq!(latest.turn_start, 91, "Latest episode turn_start must be 91");
    assert_eq!(latest.turn_end, 100, "Latest episode turn_end must be 100");

    // 9. SQLite FTS5 BM25 searchability across episodes
    let fts_hits_a = store
        .search_episodes(conv_id, fact_a_marker, 5)
        .expect("FTS5 search for Fact A must succeed");
    assert!(
        !fts_hits_a.is_empty(),
        "FTS5 search for Fact A '{}' returned 0 hits",
        fact_a_marker
    );
    assert!(
        fts_hits_a.iter().any(|ep| ep.turn_start == 1),
        "FTS5 search for Fact A must include Episode 1 (turn_start 1)"
    );

    let fts_hits_b = store
        .search_episodes(conv_id, fact_b_marker, 5)
        .expect("FTS5 search for Fact B must succeed");
    assert!(
        !fts_hits_b.is_empty(),
        "FTS5 search for Fact B '{}' returned 0 hits",
        fact_b_marker
    );
    assert!(
        fts_hits_b.iter().any(|ep| ep.turn_start == 11),
        "FTS5 search for Fact B must include Episode 2 (turn_start 11)"
    );
}

/// Asserts that a Tier 1 WorkingMemoryBuffer struct maintains its bounds.
fn assert_tier1_working_memory_buffer_sync(turn: usize, buffer: &WorkingMemoryBuffer) {
    assert!(
        buffer.turn_count() <= 8,
        "Turn {}: WorkingMemoryBuffer turn_count {} exceeds 8",
        turn,
        buffer.turn_count()
    );
    assert!(
        buffer.total_tokens() <= 2400,
        "Turn {}: WorkingMemoryBuffer total_tokens {} exceeds 2,400 limit",
        turn,
        buffer.total_tokens()
    );
    assert!(
        !buffer.exceeds_token_budget(),
        "Turn {}: WorkingMemoryBuffer exceeds_token_budget() returned true",
        turn
    );
}

// ============================================================================
// 4. 100-Turn Dialogue Generator
// ============================================================================

/// Generates realistic conversational input across 100 turns, embedding
/// Fact A at Turn 5, Fact B at Turn 20, Fact A recall at Turn 95,
/// and Fact B recall at Turn 98.
fn generate_dialogue_turn(turn: usize) -> String {
    match turn {
        5 => {
            // Fact A: Prometheus port 9464 and route /aro-metrics
            "Architecture decision: standardized on Prometheus port 9464, route /aro-metrics for all cluster telemetry. \
             Rule: remember that Prometheus port 9464, route /aro-metrics is the standard production metrics endpoint."
                .to_string()
        }
        20 => {
            // Fact B: secret ID infra-stg-7712 and cluster k8s-eu-west-3
            "Rule: remember that secret ID infra-stg-7712, cluster k8s-eu-west-3 must be used for deployment."
                .to_string()
        }
        95 => {
            // Multi-Layer Recall: Fact A query
            "Please recall Fact A: what is our standardized Prometheus port and metrics route for cluster telemetry?"
                .to_string()
        }
        98 => {
            // Multi-Layer Recall: Fact B query
            "Please recall Fact B: what secret ID and cluster must be used for deployment?"
                .to_string()
        }
        _ => {
            let topic = match turn % 10 {
                1 => "Cargo workspace configuration and dependency pruning",
                2 => "SQLite database schema migrations and WAL pragma optimization",
                3 => "Tokio async runtime task scheduling and thread worker bounds",
                4 => "Hierarchical YAML configuration loading with environment overrides",
                6 => "REST API Axum router middleware and JWT token verification",
                7 => "Token bucket rate limiter backed by Redis cache cluster",
                8 => "CORS security headers, HSTS enforcement, and CSP policies",
                9 => "Distributed tracing spans with OpenTelemetry collector export",
                0 => "Periodic health probe checklist and automated canary verification",
                _ => "General system diagnostics",
            };
            format!(
                "Turn {turn}: Reviewing {topic}. Port: {}. Endpoint: localhost:{}. Verified status.",
                8000 + (turn % 200),
                8000 + (turn % 200)
            )
        }
    }
}

// ============================================================================
// 5. Test 1: 100-Turn End-to-End Benchmark, Fact Planting & Recall
// ============================================================================

#[tokio::test]
async fn test_100_turns_end_to_end_conversation_fact_planting_and_recall() {
    let (engine, store, db_path) = create_test_harness();
    let provider = LongConversationMockProvider::new();

    let conv_id = Uuid::new_v4();
    let conv = Conversation::with_id(
        conv_id,
        "100-Turn Cognitive Memory Benchmark",
        AssistantMode::Chat,
    );
    store
        .upsert_conversation(&conv)
        .expect("must upsert conversation");

    let compactor = ContinuousCompactor::new(store.clone());
    let window_mgr = ContextWindowManager::default();
    let mut working_buffer = WorkingMemoryBuffer::new(conv_id);
    let base_system_prompt = AssistantMode::Chat.system_instruction();

    let fact_a_marker = "9464";
    let fact_b_marker = "infra-stg-7712";

    // ------------------------------------------------------------------------
    // Execute 100 Conversational Turns
    // ------------------------------------------------------------------------
    for turn in 1..=100 {
        let user_content = generate_dialogue_turn(turn);

        // 1. Send message through AssistantEngine execution loop
        let req = SendMessageRequest {
            conversation_id: Some(conv_id),
            content: user_content.clone(),
            mode: AssistantMode::Chat,
            system_prompt: None,
            model_id: Some("mock-long-conv".to_string()),
            provider: Some("mock".to_string()),
            attachments: Vec::new(),
            web_access: WebAccessMode::Off,
            search_settings: None,
            memory_settings: None,
        };

        let response = engine
            .send_message(req, &provider)
            .await
            .unwrap_or_else(|e| panic!("send_message failed at turn {}: {}", turn, e));

        // Sync Tier 1 buffer
        working_buffer.push_message(response.user_message.clone());
        working_buffer.push_message(response.assistant_message.clone());

        // 2. Fetch current persisted state
        let full_history = store
            .list_messages(conv_id)
            .expect("must list messages from sqlite");
        let semantic_candidates = store.list_pinned_memories().unwrap_or_default();
        let stored_episodes = store.list_episodes(conv_id).unwrap_or_default();
        let episodic_summaries: Vec<EpisodeSummary> =
            stored_episodes.iter().map(EpisodeSummary::from).collect();

        // 3. Assemble Context & Verify Strict Invariants
        let assembled = window_mgr
            .assemble_context(
                &base_system_prompt,
                &semantic_candidates,
                &episodic_summaries,
                &full_history,
            )
            .unwrap_or_else(|e| panic!("assemble_context failed at turn {}: {}", turn, e));

        // ASSERTION: Total context tokens <= 8,192 at EVERY turn
        assert_context_window_ceiling_invariants(turn, &assembled, window_mgr.budget());

        // ASSERTION: Working memory buffer bounded to N <= 8 turns and <= 2,400 tokens
        assert_working_memory_buffer_invariants(
            turn,
            &assembled,
            full_history.len(),
            window_mgr.budget(),
        );

        // ASSERTION: Compaction triggers every 10 turns
        assert_compaction_cadence_invariant(turn, &store, conv_id, &compactor);

        // ASSERTION: Tier 1 WorkingMemoryBuffer struct maintains invariant bounds
        assert_tier1_working_memory_buffer_sync(turn, &working_buffer);

        // 4. Specific Multi-Layer Recall Assertions
        if turn == 95 {
            // Turn 95 Query for Prometheus port 9464 and route /aro-metrics
            let assistant_text = &response.assistant_message.content;
            assert!(
                assistant_text.contains("9464"),
                "Turn 95: Assistant response MUST contain '9464'. Got: {}",
                assistant_text
            );
            assert!(
                assistant_text.contains("/aro-metrics"),
                "Turn 95: Assistant response MUST contain '/aro-metrics'. Got: {}",
                assistant_text
            );

            // Verify Fact A was evicted from raw working memory (proving recall came from Tier 2/3)
            assert!(
                assembled
                    .working_messages
                    .iter()
                    .all(|m| !m.content.contains("standardized on Prometheus port 9464")),
                "Turn 95: Fact A raw user message MUST be evicted from working memory"
            );
        }

        if turn == 98 {
            // Turn 98 Query for secret ID infra-stg-7712 and cluster k8s-eu-west-3
            let assistant_text = &response.assistant_message.content;
            assert!(
                assistant_text.contains("infra-stg-7712"),
                "Turn 98: Assistant response MUST contain 'infra-stg-7712'. Got: {}",
                assistant_text
            );
            assert!(
                assistant_text.contains("k8s-eu-west-3"),
                "Turn 98: Assistant response MUST contain 'k8s-eu-west-3'. Got: {}",
                assistant_text
            );

            // Verify Fact B was evicted from raw working memory
            assert!(
                assembled
                    .working_messages
                    .iter()
                    .all(|m| !m.content.contains("secret ID infra-stg-7712, cluster k8s-eu-west-3 must be used")),
                "Turn 98: Fact B raw user message MUST be evicted from working memory"
            );
        }
    }

    // ------------------------------------------------------------------------
    // Step 4: Verify Episode Records (Exactly 10 Episodes)
    // ------------------------------------------------------------------------
    assert_all_10_episodes_integrity(&store, conv_id, fact_a_marker, fact_b_marker);

    // ------------------------------------------------------------------------
    // Step 3 (cont): Multi-Layer Tool Recall Verification
    // ------------------------------------------------------------------------
    let run = create_test_agent_run(&store, conv_id);

    // Direct Engine Semantic Search
    let search_hits_a = engine
        .search_memories("Prometheus port 9464", 5)
        .await
        .expect("search_memories for Fact A");
    assert!(
        !search_hits_a.is_empty(),
        "search_memories for Fact A returned 0 results"
    );
    assert!(
        search_hits_a
            .iter()
            .any(|m| m.content.contains("9464") || m.content.contains("/aro-metrics")),
        "search_memories results must contain Fact A"
    );

    let search_hits_b = engine
        .search_memories("secret ID infra-stg-7712 cluster k8s-eu-west-3", 5)
        .await
        .expect("search_memories for Fact B");
    assert!(
        !search_hits_b.is_empty(),
        "search_memories for Fact B returned 0 results"
    );
    assert!(
        search_hits_b
            .iter()
            .any(|m| m.content.contains("infra-stg-7712") || m.content.contains("k8s-eu-west-3")),
        "search_memories results must contain Fact B"
    );

    // Agent Tool: memory_search (snake_case alias)
    let tool_req_search_a = ToolExecutionRequest::new(
        run.id,
        Some(conv_id),
        TOOL_MEMORY_SEARCH,
        json!({
            "query": "Prometheus port 9464",
            "limit": 5
        }),
    );
    let tool_res_search_a = engine
        .execute_tool(&run, tool_req_search_a)
        .await
        .expect("tool memory_search for Fact A");
    assert_eq!(tool_res_search_a.status, ToolExecutionStatus::Completed);
    assert!(
        tool_res_search_a.output["count"].as_u64().unwrap_or(0) >= 1,
        "tool memory_search must return at least 1 memory for Fact A"
    );

    // Agent Tool: core.memory.search (canonical dotted)
    let tool_req_search_b = ToolExecutionRequest::new(
        run.id,
        Some(conv_id),
        TOOL_CORE_MEMORY_SEARCH,
        json!({
            "query": "secret ID infra-stg-7712",
            "limit": 5
        }),
    );
    let tool_res_search_b = engine
        .execute_tool(&run, tool_req_search_b)
        .await
        .expect("tool core.memory.search for Fact B");
    assert_eq!(tool_res_search_b.status, ToolExecutionStatus::Completed);
    assert!(
        tool_res_search_b.output["count"].as_u64().unwrap_or(0) >= 1,
        "tool core.memory.search must return at least 1 memory for Fact B"
    );

    // Agent Tool: memory_recall with entity_key: "Port: 9464" (links to Episode 1)
    let tool_req_recall_a = ToolExecutionRequest::new(
        run.id,
        Some(conv_id),
        TOOL_MEMORY_RECALL,
        json!({
            "entity_key": "9464",
            "include_episodes": true
        }),
    );
    let tool_res_recall_a = engine
        .execute_tool(&run, tool_req_recall_a)
        .await
        .expect("tool memory_recall for Fact A");
    assert_eq!(tool_res_recall_a.status, ToolExecutionStatus::Completed);
    assert_eq!(tool_res_recall_a.output["found"], true);
    let related_eps_a = tool_res_recall_a
        .output
        .get("relatedEpisodes")
        .or_else(|| tool_res_recall_a.output.get("related_episodes"))
        .and_then(|v| v.as_array())
        .expect("related_episodes array");
    assert!(
        !related_eps_a.is_empty(),
        "memory_recall must return related episodes for Fact A"
    );
    assert!(
        related_eps_a.iter().any(|ep| {
            ep.get("turnStart") == Some(&json!(1)) || ep.get("turn_start") == Some(&json!(1))
        }),
        "Fact A related episode must include Episode 1 (turn_start 1)"
    );

    // Agent Tool: core.memory.recall with entity_key: "infra-stg-7712" (links to Episode 2)
    let tool_req_recall_b = ToolExecutionRequest::new(
        run.id,
        Some(conv_id),
        TOOL_CORE_MEMORY_RECALL,
        json!({
            "entity_key": "infra-stg-7712",
            "include_episodes": true
        }),
    );
    let tool_res_recall_b = engine
        .execute_tool(&run, tool_req_recall_b)
        .await
        .expect("tool core.memory.recall for Fact B");
    assert_eq!(tool_res_recall_b.status, ToolExecutionStatus::Completed);
    assert_eq!(tool_res_recall_b.output["found"], true);
    let related_eps_b = tool_res_recall_b
        .output
        .get("relatedEpisodes")
        .or_else(|| tool_res_recall_b.output.get("related_episodes"))
        .and_then(|v| v.as_array())
        .expect("related_episodes array");
    assert!(
        !related_eps_b.is_empty(),
        "core.memory.recall must return related episodes for Fact B"
    );
    assert!(
        related_eps_b.iter().any(|ep| {
            ep.get("turnStart") == Some(&json!(11)) || ep.get("turn_start") == Some(&json!(11))
        }),
        "Fact B related episode must include Episode 2 (turn_start 11)"
    );

    // ------------------------------------------------------------------------
    // Step 5: Cold Restart Resilience Verification (Turn 101)
    // ------------------------------------------------------------------------
    // 1. Drop original engine and store instances
    drop(engine);
    drop(store);

    // 2. Reopen fresh store from disk SQLite DB file
    let restarted_store =
        SqliteMemoryStore::new(&db_path).expect("reopen sqlite store after drop");
    let all_messages_restarted = restarted_store
        .list_messages(conv_id)
        .expect("list messages after restart");

    // 3. Verify ContinuousCompactor recovers state from latest episode
    let recovered_state =
        CompactionState::recover(&restarted_store, conv_id, &all_messages_restarted)
            .expect("recover compaction state");
    assert_eq!(
        recovered_state.last_compacted_turn, 100,
        "CompactionState::recover must find last_compacted_turn = 100 from latest episode"
    );
    assert_eq!(
        recovered_state.uncompacted_turns, 0,
        "CompactionState::recover must report 0 uncompacted turns at restart"
    );

    // 4. Verify all 10 episodes survived intact
    let episodes_after_restart = restarted_store
        .list_episodes(conv_id)
        .expect("list episodes after restart");
    assert_eq!(
        episodes_after_restart.len(),
        10,
        "All 10 episodes must survive cold restart intact"
    );

    // 5. Reinitialize AssistantEngine on reopened store
    let mut vector_cfg = VectorMemoryConfig::from_env();
    vector_cfg.mode = VectorMemoryMode::Disabled;
    let vector_service = MemoryVectorService::new(vector_cfg);
    let restarted_engine =
        AssistantEngine::with_vector_service(restarted_store.clone(), vector_service);

    // 6. Send message at Turn 101 querying both Fact A and Fact B
    let req101 = SendMessageRequest {
        conversation_id: Some(conv_id),
        content: "System reboot complete. Please recall both Fact A Prometheus port and Fact B secret ID and cluster.".to_string(),
        mode: AssistantMode::Chat,
        system_prompt: None,
        model_id: Some("mock-long-conv".to_string()),
        provider: Some("mock".to_string()),
        attachments: Vec::new(),
        web_access: WebAccessMode::Off,
        search_settings: None,
        memory_settings: None,
    };

    let resp101 = restarted_engine
        .send_message(req101, &provider)
        .await
        .expect("send Turn 101 after cold restart");

    // 7. Verify Turn 101 assistant response contains both facts
    let resp101_text = &resp101.assistant_message.content;
    assert!(
        resp101_text.contains("9464"),
        "Turn 101 response MUST recall Prometheus port 9464 after restart. Got: {}",
        resp101_text
    );
    assert!(
        resp101_text.contains("/aro-metrics"),
        "Turn 101 response MUST recall route /aro-metrics after restart. Got: {}",
        resp101_text
    );
    assert!(
        resp101_text.contains("infra-stg-7712"),
        "Turn 101 response MUST recall secret ID infra-stg-7712 after restart. Got: {}",
        resp101_text
    );
    assert!(
        resp101_text.contains("k8s-eu-west-3"),
        "Turn 101 response MUST recall cluster k8s-eu-west-3 after restart. Got: {}",
        resp101_text
    );

    // 8. Assert context ceiling remains strictly <= 8,192 at Turn 101
    let history_101 = restarted_store
        .list_messages(conv_id)
        .expect("list messages after turn 101");
    let pinned_101 = restarted_store
        .list_pinned_memories()
        .unwrap_or_default();
    let episodes_101 = restarted_store
        .list_episodes(conv_id)
        .unwrap_or_default();
    let summaries_101: Vec<EpisodeSummary> = episodes_101.iter().map(EpisodeSummary::from).collect();

    let assembled_101 = window_mgr
        .assemble_context(&base_system_prompt, &pinned_101, &summaries_101, &history_101)
        .expect("assemble context at turn 101");

    assert_context_window_ceiling_invariants(101, &assembled_101, window_mgr.budget());
    assert_working_memory_buffer_invariants(
        101,
        &assembled_101,
        history_101.len(),
        window_mgr.budget(),
    );

    // Cleanup
    cleanup_temp_db(db_path);
}

// ============================================================================
// 6. Test 2: Token Ceiling and Compaction Invariants with Varied Heavy Payloads
// ============================================================================

#[test]
fn test_100_turns_token_ceiling_and_compaction_invariants() {
    let (store, db_path) = {
        let path = std::env::temp_dir().join(format!(
            "aro-m4-invariants-test-{}.sqlite",
            Uuid::new_v4()
        ));
        let s = SqliteMemoryStore::new(&path).expect("create sqlite store");
        (s, path)
    };

    let conv_id = Uuid::new_v4();
    let conv = Conversation::with_id(
        conv_id,
        "Invariants Heavy Workload Benchmark",
        AssistantMode::Chat,
    );
    store.upsert_conversation(&conv).expect("upsert conv");

    let compactor = ContinuousCompactor::new(store.clone());
    let window_mgr = ContextWindowManager::default();
    let base_system_prompt = AssistantMode::Chat.system_instruction();

    for turn in 1..=100 {
        // Vary message sizes to exercise buffer bounds and ensure no partition overflow
        let (u_text, a_text) = match turn {
            5 => (
                "Architecture decision: standardized on Prometheus port 9464, route /aro-metrics for all cluster telemetry. \
                 Rule: remember that Prometheus port 9464, route /aro-metrics is the standard production metrics endpoint."
                    .to_string(),
                "Confirmed telemetry architecture on port 9464 and route /aro-metrics.".to_string(),
            ),
            20 => (
                "Rule: remember that secret ID infra-stg-7712, cluster k8s-eu-west-3 must be used for deployment. \
                 Port: 6443. Endpoint: localhost:6443."
                    .to_string(),
                "Confirmed deployment security directive for secret ID infra-stg-7712 and cluster k8s-eu-west-3 on port 6443.".to_string(),
            ),
            _ => {
                let log_snippet = format!("LOG: [2026-09-12 12:00:{:02}] module_{} worker thread processed batch with status 200 OK. Metric latency = {}ms", turn % 60, turn, 10 + (turn % 40));
                (
                    format!("Turn {turn} technical query: please optimize database indices and verify telemetry for service_{}. Port: {}. Endpoint: localhost:{}. Details: {}", turn, 8000 + (turn % 200), 8000 + (turn % 200), log_snippet),
                    format!("Turn {turn} response: Verified service_{} on port {}. Index scan eliminated, latency within SLA. Applied WAL pragma.", turn, 8000 + (turn % 200)),
                )
            }
        };

        let u_msg = ChatMessage::new(conv_id, MessageRole::User, u_text);
        store.add_message(&u_msg).expect("add u_msg");

        // Background compaction check (as runs inside engine)
        let _ = compactor.consolidate_if_needed(conv_id).expect("compaction");

        let a_msg = ChatMessage::new(conv_id, MessageRole::Assistant, a_text);
        store.add_message(&a_msg).expect("add a_msg");

        // Invariant checks
        let history = store.list_messages(conv_id).expect("list messages");
        let pinned = store.list_pinned_memories().unwrap_or_default();
        let episodes = store.list_episodes(conv_id).unwrap_or_default();
        let summaries: Vec<EpisodeSummary> = episodes.iter().map(EpisodeSummary::from).collect();

        let assembled = window_mgr
            .assemble_context(&base_system_prompt, &pinned, &summaries, &history)
            .unwrap_or_else(|e| panic!("assemble_context failed at turn {}: {}", turn, e));

        assert_context_window_ceiling_invariants(turn, &assembled, window_mgr.budget());
        assert_working_memory_buffer_invariants(
            turn,
            &assembled,
            history.len(),
            window_mgr.budget(),
        );
        assert_compaction_cadence_invariant(turn, &store, conv_id, &compactor);
    }

    assert_all_10_episodes_integrity(&store, conv_id, "9464", "infra-stg-7712");
    cleanup_temp_db(db_path);
}

// ============================================================================
// 7. Test 3: Compaction State Recovery from Mid-Session Crash
// ============================================================================

#[test]
fn test_compaction_recovery_on_interrupted_state() {
    let (store, db_path) = {
        let path = std::env::temp_dir().join(format!(
            "aro-m4-midcrash-test-{}.sqlite",
            Uuid::new_v4()
        ));
        let s = SqliteMemoryStore::new(&path).expect("create sqlite store");
        (s, path)
    };

    let conv_id = Uuid::new_v4();
    let conv = Conversation::with_id(
        conv_id,
        "Mid-Crash Compaction Recovery",
        AssistantMode::Chat,
    );
    store.upsert_conversation(&conv).expect("upsert conv");
    let compactor = ContinuousCompactor::new(store.clone());

    // Run 35 turns (3 full compactions at turns 10, 20, 30, and 5 uncompacted turns)
    for turn in 1..=35 {
        let u_msg = ChatMessage::new(
            conv_id,
            MessageRole::User,
            format!("Turn {turn}: user message"),
        );
        store.add_message(&u_msg).expect("add u");
        let _ = compactor.consolidate_if_needed(conv_id).expect("compaction");
        let a_msg = ChatMessage::new(
            conv_id,
            MessageRole::Assistant,
            format!("Turn {turn}: assistant message"),
        );
        store.add_message(&a_msg).expect("add a");
    }

    // Simulate crash and recovery from disk
    drop(store);
    let reopened = SqliteMemoryStore::new(&db_path).expect("reopen");
    let messages = reopened.list_messages(conv_id).expect("list messages");

    let state = CompactionState::recover(&reopened, conv_id, &messages)
        .expect("recover compaction state");

    assert_eq!(
        state.last_compacted_turn, 30,
        "Last compacted turn must be 30"
    );
    assert_eq!(
        state.current_turn, 35,
        "Current turn must be 35"
    );
    assert_eq!(
        state.uncompacted_turns, 5,
        "Uncompacted turns must be exactly 5 (turns 31-35)"
    );

    // Resume turns 36..=40 and verify compaction triggers at turn 40
    let resumed_compactor = ContinuousCompactor::new(reopened.clone());
    for turn in 36..=40 {
        let u_msg = ChatMessage::new(
            conv_id,
            MessageRole::User,
            format!("Turn {turn}: user message"),
        );
        reopened.add_message(&u_msg).expect("add u");
        let compaction_opt = resumed_compactor
            .consolidate_if_needed(conv_id)
            .expect("compaction");

        if turn == 40 {
            assert!(
                compaction_opt.is_some(),
                "Compaction must trigger when turn 40 completes 10 uncompacted turns"
            );
            let res = compaction_opt.unwrap();
            assert_eq!(res.episode.turn_start, 31);
            assert_eq!(res.episode.turn_end, 40);
        } else {
            assert!(compaction_opt.is_none());
        }

        let a_msg = ChatMessage::new(
            conv_id,
            MessageRole::Assistant,
            format!("Turn {turn}: assistant message"),
        );
        reopened.add_message(&a_msg).expect("add a");
    }

    let episodes = reopened.list_episodes(conv_id).expect("list episodes");
    assert_eq!(episodes.len(), 4, "Must have exactly 4 episodes after turn 40");
    assert_eq!(episodes[3].turn_start, 31);
    assert_eq!(episodes[3].turn_end, 40);

    cleanup_temp_db(db_path);
}
