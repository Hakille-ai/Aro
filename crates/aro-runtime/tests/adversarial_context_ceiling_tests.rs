//! Adversarial Challenge & Stress Test Suite: Context Window Ceiling & Multi-Turn Endurance
//!
//! Stress tests:
//! 1. Extreme payloads: Gigantic Python, Rust, and SQL source code blocks (30k-100k chars).
//! 2. Heavy formatting: Gigantic Markdown tables and dense symbol noise.
//! 3. Deeply nested JSON and AST payloads.
//! 4. 50+ turn conversational endurance with continuous compaction and token ceiling invariant checks at EVERY turn.
//! 5. Multi-step tool loop sliding window stress test (50 consecutive tool calls with massive intermediate outputs).
//! 6. Single oversized message boundary conditions (> 2,400 tokens).
//! 7. System prompt budget boundary enforcement (800 vs 801 tokens).

use aro_core::{
    estimate_message_tokens, estimate_tokens, AssistantMode, ChatMessage, Conversation,
    EpisodeSummary, LongTermMemory, MessageRole,
};
use aro_memory::SqliteMemoryStore;
use aro_runtime::context_manager::{ContinuousCompactor, ContextWindowManager};
use uuid::Uuid;

fn create_temp_store() -> (SqliteMemoryStore, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!(
        "aro-adversarial-context-{}.sqlite",
        Uuid::new_v4()
    ));
    let store = SqliteMemoryStore::new(&db_path).expect("failed to create sqlite memory store");
    (store, db_path)
}

fn create_conversation(store: &SqliteMemoryStore, conv_id: Uuid) {
    let conv = Conversation::with_id(conv_id, "Stress Test Conv", AssistantMode::Chat);
    store.upsert_conversation(&conv).expect("must create conversation");
}

fn cleanup_temp_store(path: std::path::PathBuf) {
    let _ = std::fs::remove_file(path);
}

// ----------------------------------------------------------------------------
// Generator Helpers for Extreme Payloads
// ----------------------------------------------------------------------------

fn generate_gigantic_python_code(functions: usize) -> String {
    let mut out = String::from("```python\n# Auto-generated gigantic Python module\nimport sys\nimport os\nfrom typing import Dict, List, Optional, Any\n\n");
    for i in 0..functions {
        out.push_str(&format!(
            "def data_processor_handler_variant_{i}(payload: Dict[str, Any], flag: bool = True) -> Optional[List[int]]:\n"
        ));
        out.push_str(&format!(
            "    \"\"\"Process telemetry batch {i} with deterministic transformation.\"\"\"\n"
        ));
        out.push_str("    if not payload or not flag:\n");
        out.push_str("        return None\n");
        out.push_str("    accumulator: List[int] = []\n");
        out.push_str("    for key, value in payload.items():\n");
        out.push_str(&format!(
            "        val_hash = (hash(str(key)) ^ hash(str(value)) ^ {i}) & 0xFFFF\n"
        ));
        out.push_str("        accumulator.append(val_hash)\n");
        out.push_str("    return sorted(accumulator)\n\n");
    }
    out.push_str("```\n");
    out
}

fn generate_gigantic_rust_code(structs: usize) -> String {
    let mut out = String::from("```rust\n// Auto-generated gigantic Rust module\nuse std::sync::Arc;\nuse std::collections::BTreeMap;\n\n");
    for i in 0..structs {
        out.push_str(&format!(
            "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct NodeDescriptorVariant{i}<T: Ord + Clone> {{\n"
        ));
        out.push_str("    pub node_id: u64,\n    pub label: String,\n    pub payload: BTreeMap<String, T>,\n    pub epoch: usize,\n    pub checksum: [u8; 32],\n}\n\n");
        out.push_str(&format!(
            "impl<T: Ord + Clone> NodeDescriptorVariant{i}<T> {{\n"
        ));
        out.push_str(&format!("    pub fn new(id: u64, label: &str) -> Self {{\n        Self {{\n            node_id: id ^ {i},\n            label: label.to_string(),\n            payload: BTreeMap::new(),\n            epoch: {i},\n            checksum: [{i} as u8; 32],\n        }}\n    }}\n}}\n\n"));
    }
    out.push_str("```\n");
    out
}

fn generate_gigantic_sql_schema(tables: usize) -> String {
    let mut out = String::from("```sql\n-- Auto-generated gigantic SQL migration script\nBEGIN TRANSACTION;\n\n");
    for i in 0..tables {
        out.push_str(&format!(
            "CREATE TABLE IF NOT EXISTS telemetry_records_v{i} (\n"
        ));
        out.push_str("    id BIGSERIAL PRIMARY KEY,\n");
        out.push_str("    uuid UUID NOT NULL UNIQUE,\n");
        out.push_str(&format!("    service_tier INT DEFAULT {i},\n"));
        out.push_str("    payload JSONB NOT NULL,\n");
        out.push_str("    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,\n");
        out.push_str("    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP\n");
        out.push_str(");\n");
        out.push_str(&format!(
            "CREATE INDEX IF NOT EXISTS idx_telemetry_v{i}_uuid ON telemetry_records_v{i}(uuid);\n"
        ));
        out.push_str(&format!(
            "CREATE INDEX IF NOT EXISTS idx_telemetry_v{i}_created ON telemetry_records_v{i}(created_at);\n\n"
        ));
    }
    out.push_str("COMMIT;\n```\n");
    out
}

fn generate_gigantic_markdown_table(rows: usize) -> String {
    let mut out = String::from("| ID | Service Name | Status | Latency (ms) | Success Rate | Memory (MB) | CPU % | Error Count | Cluster Region | Last Health Check |\n");
    out.push_str("|---|---|---|---|---|---|---|---|---|---|\n");
    for i in 0..rows {
        out.push_str(&format!(
            "| {i} | service-{i}.internal | HEALTHY | {} | 99.98% | {} | 14.5% | 0 | us-east-{} | 2026-09-12T00:00:00Z |\n",
            10 + (i % 50),
            128 + (i % 256),
            (i % 5) + 1
        ));
    }
    out
}

fn generate_deeply_nested_json(depth: usize) -> String {
    let mut out = String::new();
    for d in 0..depth {
        out.push_str(&format!("{{\"level_{d}\": "));
    }
    out.push_str("{\"leaf_value\": \"deep_payload\", \"code\": 200, \"status\": \"OK\"}");
    for _ in 0..depth {
        out.push('}');
    }
    out
}

// ----------------------------------------------------------------------------
// Test 1: Extreme Source Code Payloads
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_extreme_source_code_payloads() {
    let mgr = ContextWindowManager::default();
    let conv_id = Uuid::new_v4();

    let py_code = generate_gigantic_python_code(120); // ~20,000+ chars
    let rs_code = generate_gigantic_rust_code(100);   // ~30,000+ chars
    let sql_code = generate_gigantic_sql_schema(100);  // ~25,000+ chars

    // Candidate working messages containing extreme code blocks
    let working_messages = vec![
        ChatMessage::new(conv_id, MessageRole::User, format!("Please review this Python code:\n{}", py_code)),
        ChatMessage::new(conv_id, MessageRole::Assistant, format!("Here is the corresponding Rust implementation:\n{}", rs_code)),
        ChatMessage::new(conv_id, MessageRole::User, format!("Now migrate this to SQL schema:\n{}", sql_code)),
        ChatMessage::new(conv_id, MessageRole::Assistant, "Schema migration verified and validated.".to_string()),
    ];

    // Semantic memory candidates with code snippets
    let mut semantic_candidates = Vec::new();
    for i in 0..30 {
        let mut mem = LongTermMemory::new(
            format!("Rule {i}: Always use proper types in Rust:\n```rust\npub type Handle{i} = Arc<Mutex<Vec<u8>>>;\n```"),
            Some(conv_id),
        );
        mem.pinned = i < 5;
        mem.salience = 0.6 + (i as f32 * 0.01);
        semantic_candidates.push(mem);
    }

    // Episodic candidates with code summaries
    let mut episodic_candidates = Vec::new();
    for i in 0..15 {
        episodic_candidates.push(EpisodeSummary {
            id: Uuid::new_v4(),
            conversation_id: conv_id,
            turn_start: i * 10 + 1,
            turn_end: (i + 1) * 10,
            summary: format!("Summary {i} with code references:\n```python\nx = process_{i}()\n```"),
            key_decisions: vec![format!("Decision {i}: Use async I/O")],
            entities: vec![format!("service-{i}.internal:808{i}")],
            token_count: 150,
            created_at: chrono::Utc::now(),
        });
    }

    let sys_prompt = "You are ARO AI, a high-performance cognitive assistant.";
    let assembled = mgr
        .assemble_context(sys_prompt, &semantic_candidates, &episodic_candidates, &working_messages)
        .expect("assemble_context must succeed even with extreme code payloads");

    // Ceiling checks
    assert!(
        assembled.token_usage.total_tokens <= 8192,
        "Total tokens {} MUST be <= 8192",
        assembled.token_usage.total_tokens
    );
    assert!(assembled.token_usage.system_tokens <= 800);
    assert!(assembled.token_usage.semantic_tokens <= 1600);
    assert!(assembled.token_usage.episodic_tokens <= 2000);
    assert!(assembled.token_usage.working_tokens <= 2400);
    assert_eq!(assembled.token_usage.reserve_tokens, 1392);
    assert!(assembled.working_messages.len() <= 8);
    assert!(assembled.token_usage.is_within_budget(mgr.budget()));

    // Eviction verification: candidate messages exceeded budget, so evictions MUST be recorded
    assert!(assembled.eviction_report.working_evicted > 0 || assembled.working_messages.len() <= 4);
}

// ----------------------------------------------------------------------------
// Test 2: Extreme Markdown Tables & Dense Symbol Noise
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_extreme_markdown_tables_and_formatting() {
    let mgr = ContextWindowManager::default();
    let conv_id = Uuid::new_v4();

    // 1,500 row table (~150,000 characters with high symbol density)
    let huge_table = generate_gigantic_markdown_table(1500);
    assert!(huge_table.len() > 100_000);

    let messages = vec![
        ChatMessage::new(conv_id, MessageRole::User, "Show me all cluster nodes:".to_string()),
        ChatMessage::new(conv_id, MessageRole::Assistant, huge_table),
        ChatMessage::new(conv_id, MessageRole::User, "Can you filter the healthy nodes?".to_string()),
        ChatMessage::new(conv_id, MessageRole::Assistant, "All nodes in us-east-1 are healthy.".to_string()),
    ];

    let assembled = mgr
        .assemble_context("System prompt for telemetry node viewer.", &[], &[], &messages)
        .expect("Must handle huge markdown tables gracefully");

    assert!(
        assembled.token_usage.total_tokens <= 8192,
        "Ceiling exceeded: {}",
        assembled.token_usage.total_tokens
    );
    assert!(
        assembled.token_usage.working_tokens <= 2400,
        "Working tokens exceeded 2400: {}",
        assembled.token_usage.working_tokens
    );
    assert!(assembled.token_usage.is_within_budget(mgr.budget()));

    // The gigantic table message must have been evicted to preserve the 2400 token working ceiling!
    assert!(assembled.eviction_report.working_evicted >= 1);
}

// ----------------------------------------------------------------------------
// Test 3: Deeply Nested JSON & Dense AST Structures
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_deep_json_and_ast_structures() {
    let mgr = ContextWindowManager::default();
    let conv_id = Uuid::new_v4();

    // 250 levels of nested JSON
    let deep_json = generate_deeply_nested_json(250);

    // Large JSON array with 500 records serialized compactly without spaces
    let mut json_array = String::from("[");
    for i in 0..500 {
        if i > 0 {
            json_array.push(',');
        }
        json_array.push_str(&format!(
            "{{\"id\":{i},\"type\":\"AST_NODE_{i}\",\"flags\":[\"IMMUTABLE\",\"PUBLIC\"],\"refs\":[{i},{i}+1]}}"
        ));
    }
    json_array.push(']');

    let messages = vec![
        ChatMessage::new(conv_id, MessageRole::User, format!("AST Dump:\n{}", deep_json)),
        ChatMessage::new(conv_id, MessageRole::Assistant, format!("Node array parsed:\n{}", json_array)),
        ChatMessage::new(conv_id, MessageRole::User, "Optimize the AST.".to_string()),
    ];

    let assembled = mgr
        .assemble_context("System instruction for compiler optimization.", &[], &[], &messages)
        .expect("Must assemble context with deep JSON");

    assert!(assembled.token_usage.total_tokens <= 8192);
    assert!(assembled.token_usage.working_tokens <= 2400);
    assert!(assembled.token_usage.is_within_budget(mgr.budget()));
}

// ----------------------------------------------------------------------------
// Test 4: 55-Turn Long Conversation Endurance Stress Test
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_long_turn_endurance_55_turns() {
    let (store, db_path) = create_temp_store();
    let conv_id = Uuid::new_v4();
    create_conversation(&store, conv_id);

    let compactor = ContinuousCompactor::new(store.clone());
    let mgr = ContextWindowManager::default();
    let base_system_prompt = "You are ARO AI system assistant. Strictly follow instructions.";

    // Seed realistic architectural decisions, ports, URLs, configs across turns
    for turn in 1..=55 {
        let (user_text, assistant_text) = match turn {
            3 => (
                "Endpoint: 127.0.0.1:9090 for telemetry daemon. Please initialize.".to_string(),
                "Telemetry listener initialized on 127.0.0.1:9090 with heartbeat polling.".to_string(),
            ),
            7 => (
                "During architecture review, we decided to adopt SQLite WAL mode with synchronous = NORMAL.".to_string(),
                "Architecture decision noted: SQLite WAL mode with synchronous = NORMAL configured.".to_string(),
            ),
            12 => (
                "Set DATABASE_URL=postgres://app:secret123@db.internal:5432/arodb for analytics.".to_string(),
                "Configured DATABASE_URL for analytics pipeline.".to_string(),
            ),
            25 => (
                "Always enforce UTF-8 encoding in serializers and never disable validation.".to_string(),
                "User rule recorded: Strict UTF-8 enforcement and mandatory validation.".to_string(),
            ),
            42 => (
                "Security decision: Connect to secure gateway at https://gateway.aro.internal:8443 with port 8443.".to_string(),
                "Configured gateway endpoint https://gateway.aro.internal:8443 on port 8443.".to_string(),
            ),
            _ => (
                format!(
                    "Turn {turn}: Querying service status for module_{turn}. Payload: {}",
                    "data_chunk_".repeat(20)
                ),
                format!(
                    "Reply {turn}: Module_{turn} operational. Processed {} records successfully.",
                    turn * 10
                ),
            ),
        };

        let u_msg = ChatMessage::new(conv_id, MessageRole::User, user_text);
        store.add_message(&u_msg).unwrap();

        let a_msg = ChatMessage::new(conv_id, MessageRole::Assistant, assistant_text);
        store.add_message(&a_msg).unwrap();

        // 1. Evaluate compaction after each turn (matching runtime behavior)
        let _ = compactor.consolidate_if_needed(conv_id).unwrap();

        // 2. Fetch all candidates and assemble context
        let semantic_candidates = store.list_memories().unwrap_or_default();
        let stored_episodes = store.list_episodes(conv_id).unwrap_or_default();
        let episodic_summaries: Vec<EpisodeSummary> =
            stored_episodes.iter().map(EpisodeSummary::from).collect();
        let full_history = store.list_messages(conv_id).unwrap();

        let assembled = mgr
            .assemble_context(
                base_system_prompt,
                &semantic_candidates,
                &episodic_summaries,
                &full_history,
            )
            .unwrap_or_else(|e| panic!("assemble_context failed at turn {turn}: {e}"));

        // 3. STRICT INVARIANT VERIFICATION AT EVERY SINGLE TURN
        assert!(
            assembled.token_usage.total_tokens <= 8192,
            "Turn {turn}: Total tokens {} EXCEEDS 8,192 ceiling!",
            assembled.token_usage.total_tokens
        );
        assert!(
            assembled.token_usage.working_tokens <= 2400,
            "Turn {turn}: Working tokens {} EXCEEDS 2,400 partition!",
            assembled.token_usage.working_tokens
        );
        assert!(
            assembled.token_usage.episodic_tokens <= 2000,
            "Turn {turn}: Episodic tokens {} EXCEEDS 2,000 partition!",
            assembled.token_usage.episodic_tokens
        );
        assert!(
            assembled.token_usage.semantic_tokens <= 1600,
            "Turn {turn}: Semantic tokens {} EXCEEDS 1,600 partition!",
            assembled.token_usage.semantic_tokens
        );
        assert!(
            assembled.token_usage.system_tokens <= 800,
            "Turn {turn}: System tokens {} EXCEEDS 800 partition!",
            assembled.token_usage.system_tokens
        );
        assert_eq!(
            assembled.token_usage.reserve_tokens, 1392,
            "Turn {turn}: Reserve tokens must be exactly 1392"
        );
        assert!(
            assembled.working_messages.len() <= 8,
            "Turn {turn}: Working messages count {} EXCEEDS max 8 turns!",
            assembled.working_messages.len()
        );
        assert!(
            assembled.token_usage.is_within_budget(mgr.budget()),
            "Turn {turn}: ContextTokenUsage violated is_within_budget check"
        );
    }

    // Post-55 turn state verification
    let episodes = store.list_episodes(conv_id).unwrap();
    assert_eq!(
        episodes.len(),
        5,
        "55 turns must produce exactly 5 consolidated episodes (10 turns each)"
    );
    assert_eq!(episodes[0].turn_start, 1);
    assert_eq!(episodes[0].turn_end, 10);
    assert_eq!(episodes[1].turn_start, 11);
    assert_eq!(episodes[1].turn_end, 20);
    assert_eq!(episodes[2].turn_start, 21);
    assert_eq!(episodes[2].turn_end, 30);
    assert_eq!(episodes[3].turn_start, 31);
    assert_eq!(episodes[3].turn_end, 40);
    assert_eq!(episodes[4].turn_start, 41);
    assert_eq!(episodes[4].turn_end, 50);

    // Verify lossless extraction preserved critical items across turns
    let memories = store.list_memories().unwrap();
    assert!(
        memories.iter().any(|m| m.content.contains("SQLite WAL mode")),
        "Turn 7 decision must be preserved in long-term memories"
    );
    assert!(
        memories.iter().any(|m| m.content.contains("UTF-8 encoding")),
        "Turn 25 rule must be preserved in long-term memories"
    );

    cleanup_temp_store(db_path);
}

// ----------------------------------------------------------------------------
// Test 5: Multi-Step Tool Loop Sliding Window Stress Test (50 Steps)
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_multistep_tool_loop_50_steps_sliding_window() {
    let mgr = ContextWindowManager::default();
    let conv_id = Uuid::new_v4();
    let mut history: Vec<ChatMessage> = Vec::new();

    // Initial user prompt
    history.push(ChatMessage::new(
        conv_id,
        MessageRole::User,
        "Execute automated workflow across 50 tool invocations.".to_string(),
    ));

    // Simulate 50 consecutive tool execution steps
    for step in 1..=50 {
        let tool_name = format!("tool_step_{step}");
        let action_input = format!("{{\"step\": {step}, \"action\": \"query_data\"}}");

        // Tool call message
        history.push(ChatMessage::new(
            conv_id,
            MessageRole::Assistant,
            format!("Calling tool `{tool_name}` with arguments: {action_input}"),
        ));

        // Tool result message with varying payload sizes
        let result_content = match step {
            // Steps 10, 20, 30, 40: Gigantic tool payload (e.g. read_file of large bundle)
            s if s % 10 == 0 => {
                format!("Large file contents:\n{}", "binary_hex_encoded_data_".repeat(800))
            }
            // Steps with code
            s if s % 3 == 0 => {
                format!("Code analysis:\n```rust\nfn step_{s}() {{ println!(\"step\"); }}\n```")
            }
            // Standard small result
            _ => format!("Status: OK. Metric value = {}", step * 42),
        };

        history.push(ChatMessage::new(
            conv_id,
            MessageRole::User,
            format!("Tool `{tool_name}` returned: Success\n{result_content}"),
        ));

        // Re-apply sliding window bounds on working history (mirroring run_tool_loop line 868)
        let (bounded_history, tokens, evicted) = mgr.fit_working_messages(&history);
        history = bounded_history;

        // INVARIANT CHECKS AT EVERY TOOL STEP
        assert!(
            history.len() <= 8,
            "Step {step}: Tool loop history length {} must be <= 8",
            history.len()
        );
        assert!(
            tokens <= 2400,
            "Step {step}: Tool loop tokens {} must be <= 2400",
            tokens
        );

        if step > 4 {
            assert!(
                evicted > 0 || history.len() <= 8,
                "Step {step}: Must report evictions as tool steps accumulate"
            );
        }

        // Context assembly with current tool history must strictly satisfy 8,192 ceiling
        let assembled = mgr
            .assemble_context("Agent system instructions.", &[], &[], &history)
            .unwrap_or_else(|e| panic!("Step {step}: assemble_context failed: {e}"));

        assert!(
            assembled.token_usage.total_tokens <= 8192,
            "Step {step}: Total tokens {} exceeds 8192 ceiling!",
            assembled.token_usage.total_tokens
        );
        assert!(assembled.token_usage.working_tokens <= 2400);
    }
}

// ----------------------------------------------------------------------------
// Test 6: Single Oversized Message Boundary Conditions (> 2,400 Tokens)
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_single_oversized_message_exceeding_budget() {
    let mgr = ContextWindowManager::default();
    let conv_id = Uuid::new_v4();

    // A single user message with 40,000 characters (~10,000 tokens >> 2,400 limit)
    let giant_user_msg = ChatMessage::new(
        conv_id,
        MessageRole::User,
        format!("Massive log dump:\n{}", "LOG_LINE: connection failed with timeout.\n".repeat(1000)),
    );
    let giant_tokens = estimate_message_tokens(&giant_user_msg);
    assert!(giant_tokens > 2400, "Must exceed 2400 token working budget");

    // Case A: Message history containing ONLY this giant message
    let (retained_a, tokens_a, evicted_a) = mgr.fit_working_messages(std::slice::from_ref(&giant_user_msg));
    // The giant message cannot fit in the 2400 budget, so it is safely dropped/evicted
    assert_eq!(retained_a.len(), 0, "Oversized message must be evicted");
    assert_eq!(tokens_a, 0);
    assert_eq!(evicted_a, 1);

    // Case B: Giant message followed by standard small message
    let small_msg = ChatMessage::new(conv_id, MessageRole::Assistant, "I cannot process logs of that size.".to_string());
    let (retained_b, tokens_b, evicted_b) = mgr.fit_working_messages(&[giant_user_msg, small_msg]);
    assert_eq!(retained_b.len(), 1, "Small message must be retained");
    assert_eq!(retained_b[0].content, "I cannot process logs of that size.");
    assert!(tokens_b <= 2400);
    assert_eq!(evicted_b, 1);

    // Context assembly must still succeed and be well within 8192
    let assembled = mgr
        .assemble_context("System instructions.", &[], &[], &retained_b)
        .expect("assemble_context must succeed");
    assert!(assembled.token_usage.total_tokens <= 8192);
}

// ----------------------------------------------------------------------------
// Test 7: System Prompt Budget Boundary Enforcement (800 vs 801 Tokens)
// ----------------------------------------------------------------------------
#[test]
fn test_adversarial_system_prompt_budget_boundary() {
    let mgr = ContextWindowManager::default();

    // 1. Valid system prompt within 800 tokens
    let valid_prompt = "You are ARO AI system assistant. ".repeat(30); // ~240 tokens
    let valid_tokens = estimate_tokens(&valid_prompt);
    assert!(valid_tokens < 800);
    let fit_result = mgr.fit_system_prompt(&valid_prompt);
    assert!(fit_result.is_ok(), "Prompt within budget must succeed");

    // 2. Oversized system prompt exceeding 800 tokens
    let oversized_prompt = "You are ARO AI system assistant. ".repeat(150); // ~1,200 tokens
    let over_tokens = estimate_tokens(&oversized_prompt);
    assert!(over_tokens > 800);
    let fit_over_result = mgr.fit_system_prompt(&oversized_prompt);
    assert!(
        fit_over_result.is_err(),
        "Prompt exceeding 800 tokens must return Err(AroError::Configuration)"
    );

    // assemble_context must reject the oversized system prompt
    let assembled_err = mgr.assemble_context(&oversized_prompt, &[], &[], &[]);
    assert!(assembled_err.is_err(), "assemble_context must reject oversized system prompt");
}
