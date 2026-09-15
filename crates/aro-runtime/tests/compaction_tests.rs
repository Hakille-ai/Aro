//! Milestone 2 Integration & Unit Test Suite: Adaptive Context Management & Lossless Compactor
//!
//! Tests:
//! - 10-turn compaction interval trigger
//! - 2,400 token threshold compaction trigger
//! - Lossless extraction: ports, URLs, configs/secrets, architectural decisions, user rules/preferences
//! - Strict 8,192 token window ceiling enforcement across all partitions
//! - Sliding window reverse-recency bounds on working memory (N <= 8, tokens <= 2400)
//! - Cold restart resilience and state recovery from SQLite
//! - Multi-interval compaction across 30+ turns

use aro_core::{
    ChatMessage, Conversation, EpisodeSummary, LongTermMemory, MessageRole, AssistantMode,
};
use aro_memory::SqliteMemoryStore;
use aro_runtime::context_manager::{
    extract_configs_and_keys, extract_decisions, extract_ports, extract_urls_and_uris,
    extract_user_rules, CompactionState, ContinuousCompactor, ContextWindowManager,
};
use uuid::Uuid;

fn create_temp_store() -> (SqliteMemoryStore, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!("aro-compaction-test-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&db_path).expect("failed to create sqlite memory store");
    (store, db_path)
}

fn create_conversation(store: &SqliteMemoryStore, conv_id: Uuid) {
    let conv = Conversation::with_id(conv_id, "Test Conversation", AssistantMode::Chat);
    store.upsert_conversation(&conv).expect("must create conversation");
}

fn cleanup_temp_store(path: std::path::PathBuf) {
    let _ = std::fs::remove_file(path);
}

#[test]
fn test_lossless_extraction_ports_and_endpoints() {
    let input = "The backend runs on port 8080. Connect to localhost:3000, 127.0.0.1:5432, 0.0.0.0:8000, and internal-service.internal:9090. Ignore invalid port 99999.";
    let ports = extract_ports(input);

    assert!(ports.contains(&"Port: 8080".to_string()), "Must extract Port: 8080");
    assert!(
        ports.contains(&"Endpoint: localhost:3000".to_string()),
        "Must extract Endpoint: localhost:3000"
    );
    assert!(
        ports.contains(&"Endpoint: 127.0.0.1:5432".to_string()),
        "Must extract Endpoint: 127.0.0.1:5432"
    );
    assert!(
        ports.contains(&"Endpoint: 0.0.0.0:8000".to_string()),
        "Must extract Endpoint: 0.0.0.0:8000"
    );
    assert!(
        ports.contains(&"Endpoint: internal-service.internal:9090".to_string()),
        "Must extract Endpoint: internal-service.internal:9090"
    );
    assert!(
        !ports.contains(&"Port: 99999".to_string()),
        "Must reject out-of-range port"
    );
}

#[test]
fn test_lossless_extraction_urls_and_uris() {
    let input = "Check documentation at https://aro.dev/docs and ws://live.aro.internal:4000. Database is at postgres://user:secret@localhost:5432/arodb and redis://cache:6379.";
    let urls = extract_urls_and_uris(input);

    assert!(
        urls.iter().any(|u| u.contains("https://aro.dev/docs")),
        "Must extract https URL"
    );
    assert!(
        urls.iter().any(|u| u.contains("ws://live.aro.internal:4000")),
        "Must extract ws URI"
    );
    assert!(
        urls.iter().any(|u| u.contains("postgres://user:secret@localhost:5432/arodb")),
        "Must extract postgres URI"
    );
    assert!(
        urls.iter().any(|u| u.contains("redis://cache:6379")),
        "Must extract redis URI"
    );
}

#[test]
fn test_lossless_extraction_configs_and_secrets() {
    let input = "Set DATABASE_URL=postgres://localhost/db and NODE_ENV=production. Use OpenAI key sk-proj-1234567890abcdef1234567890 and GitHub token ghp_123456789012345678901234567890123456.";
    let configs = extract_configs_and_keys(input);

    assert!(
        configs.iter().any(|c| c.contains("DATABASE_URL=")),
        "Must extract DATABASE_URL config"
    );
    assert!(
        configs.iter().any(|c| c.contains("NODE_ENV=production")),
        "Must extract NODE_ENV config"
    );
    assert!(
        configs.iter().any(|c| c.contains("sk-proj-")),
        "Must extract OpenAI secret key"
    );
    assert!(
        configs.iter().any(|c| c.contains("ghp_")),
        "Must extract GitHub secret key"
    );
}

#[test]
fn test_lossless_extraction_architectural_decisions() {
    let input = "During architecture review, we decided to adopt SQLite WAL mode for high concurrency. Architecture decision: Standardized on Reciprocal Rank Fusion.";
    let decisions = extract_decisions(input);

    assert!(!decisions.is_empty(), "Must extract decisions");
    assert!(
        decisions.iter().any(|d| d.contains("SQLite WAL mode")),
        "Must capture SQLite WAL decision"
    );
    assert!(
        decisions.iter().any(|d| d.contains("Reciprocal Rank Fusion")),
        "Must capture RRF architecture decision"
    );
}

#[test]
fn test_lossless_extraction_user_rules_and_preferences() {
    let input = "Always use strict typing in Rust. Never disable auth checks in production. I prefer dark theme in UI. Remember that user wants concise explanations.";
    let rules = extract_user_rules(input);

    assert!(!rules.is_empty(), "Must extract rules");
    assert!(
        rules.iter().any(|r| r.contains("Always use strict typing")),
        "Must capture 'Always' rule"
    );
    assert!(
        rules.iter().any(|r| r.contains("Never disable auth")),
        "Must capture 'Never' rule"
    );
    assert!(
        rules.iter().any(|r| r.contains("dark theme")),
        "Must capture preference"
    );
    assert!(
        rules.iter().any(|r| r.contains("concise explanations")),
        "Must capture 'Remember' directive"
    );
}

#[test]
fn test_compaction_turn_interval_trigger_at_10_turns() {
    let (store, db_path) = create_temp_store();
    let conv_id = Uuid::new_v4();
    create_conversation(&store, conv_id);

    // Populate 10 conversational turns with realistic message volume
    for i in 1..=10 {
        let u_msg = ChatMessage::new(
            conv_id,
            MessageRole::User,
            format!(
                "Turn {}: We decided to configure service_{} on port {}. Detailed specifications: Component must handle high-throughput telemetry streams, maintain local SQLite connection pools with PRAGMA synchronous = NORMAL, and implement robust retry backoff on concurrency conflicts. Please confirm setup and return the status report for deployment.",
                i, i, 8000 + i
            ),
        );
        store.add_message(&u_msg).unwrap();

        let a_msg = ChatMessage::new(
            conv_id,
            MessageRole::Assistant,
            format!(
                "Response {}: Service_{} has been initialized and bound to port {}. SQLite WAL configuration is verified with PRAGMA journal_mode = WAL and synchronous = NORMAL. Transaction isolation is set to immediate and connection pooling is active. Telemetry stream listeners are registered, health checks report passing status, and backoff handlers are active. Deployment ready.",
                i, i, 8000 + i
            ),
        );
        store.add_message(&a_msg).unwrap();
    }

    let compactor = ContinuousCompactor::new(store.clone());
    let compaction_opt = compactor.consolidate_if_needed(conv_id).unwrap();

    assert!(compaction_opt.is_some(), "Compaction must trigger after 10 turns");
    let result = compaction_opt.unwrap();
    assert_eq!(result.turns_compacted, 10);
    assert_eq!(result.episode.turn_start, 1);
    assert_eq!(result.episode.turn_end, 10);
    assert!(result.tokens_after < result.tokens_before);

    // Verify episode persisted in SQLite
    let latest_ep = store.get_latest_episode(conv_id).unwrap();
    assert!(latest_ep.is_some(), "Episode must be stored in SQLite");
    let ep = latest_ep.unwrap();
    assert_eq!(ep.turn_start, 1);
    assert_eq!(ep.turn_end, 10);
    assert!(!ep.entities.is_empty(), "Episode must contain extracted entities");

    // Check extracted high-salience memories persisted in memories table
    let memories = store.list_memories().unwrap();
    assert!(!memories.is_empty(), "Proposed memories must be stored in memories table");

    // Second evaluation with no new turns should skip
    let second_eval = compactor.consolidate_if_needed(conv_id).unwrap();
    assert!(second_eval.is_none(), "Must skip when no uncompacted turns remain");

    cleanup_temp_store(db_path);
}

#[test]
fn test_compaction_token_threshold_trigger() {
    let (store, db_path) = create_temp_store();
    let conv_id = Uuid::new_v4();
    create_conversation(&store, conv_id);

    // Populate 3 turns with large payload exceeding 2400 tokens (~10,000 characters)
    for i in 1..=3 {
        let u_msg = ChatMessage::new(
            conv_id,
            MessageRole::User,
            format!("Turn {}: Big payload {}", i, "x".repeat(4000)),
        );
        store.add_message(&u_msg).unwrap();

        let a_msg = ChatMessage::new(
            conv_id,
            MessageRole::Assistant,
            format!("Turn {} response: {}", i, "y".repeat(4000)),
        );
        store.add_message(&a_msg).unwrap();
    }

    let compactor = ContinuousCompactor::new(store.clone());
    let compaction_opt = compactor.consolidate_if_needed(conv_id).unwrap();

    assert!(compaction_opt.is_some(), "Must trigger compaction when uncompacted tokens >= 2400");
    let result = compaction_opt.unwrap();
    assert_eq!(result.turns_compacted, 3);
    assert_eq!(result.episode.turn_start, 1);
    assert_eq!(result.episode.turn_end, 3);

    cleanup_temp_store(db_path);
}

#[test]
fn test_context_window_manager_8192_strict_ceiling() {
    let mgr = ContextWindowManager::default();
    let conv_id = Uuid::new_v4();

    // Candidate semantic memories (20 items)
    let mut semantic_candidates = Vec::new();
    for i in 1..=20 {
        let mut mem = LongTermMemory::new(
            format!("Semantic rule {}: Always validate data before writing to database.", i),
            Some(conv_id),
        );
        mem.pinned = i <= 3; // First 3 pinned
        mem.salience = 0.5 + (i as f32 * 0.02);
        semantic_candidates.push(mem);
    }

    // Candidate episodic summaries (10 items)
    let mut episodic_candidates = Vec::new();
    for i in 1..=10 {
        episodic_candidates.push(EpisodeSummary {
            id: Uuid::new_v4(),
            conversation_id: conv_id,
            turn_start: (i - 1) * 10 + 1,
            turn_end: i * 10,
            summary: format!("Summary of turns {}-{}: Setup architecture and database.", (i - 1) * 10 + 1, i * 10),
            key_decisions: vec![format!("Decision chunk {}", i)],
            entities: vec!["SQLite".to_string(), "Postgres".to_string()],
            token_count: 80,
            created_at: chrono::Utc::now(),
        });
    }

    // Candidate working messages (25 messages)
    let mut working_messages = Vec::new();
    for i in 1..=25 {
        working_messages.push(ChatMessage::new(
            conv_id,
            if i % 2 == 1 { MessageRole::User } else { MessageRole::Assistant },
            format!("Working turn {}: Detail information and instructions.", i),
        ));
    }

    let sys_prompt = "You are ARO AI system assistant. Strictly follow instructions.";
    let assembled = mgr
        .assemble_context(sys_prompt, &semantic_candidates, &episodic_candidates, &working_messages)
        .expect("assemble_context must succeed");

    // Partition invariants
    assert!(assembled.token_usage.system_tokens <= 800, "System tokens <= 800");
    assert!(assembled.token_usage.semantic_tokens <= 1600, "Semantic tokens <= 1600");
    assert!(assembled.token_usage.episodic_tokens <= 2000, "Episodic tokens <= 2000");
    assert!(assembled.token_usage.working_tokens <= 2400, "Working tokens <= 2400");
    assert_eq!(assembled.token_usage.reserve_tokens, 1392, "Reserve headroom = 1392");
    assert!(
        assembled.token_usage.total_tokens <= 8192,
        "Total tokens {} MUST be <= 8192 ceiling",
        assembled.token_usage.total_tokens
    );

    // Working buffer invariants
    assert!(
        assembled.working_messages.len() <= 8,
        "Working messages count {} must be <= 8",
        assembled.working_messages.len()
    );

    // Eviction report verification
    assert!(assembled.eviction_report.working_evicted >= 17);
    assert_eq!(assembled.eviction_report.working_retained, assembled.working_messages.len());
}

#[test]
fn test_sliding_window_reverse_recency() {
    let mgr = ContextWindowManager::default();
    let conv_id = Uuid::new_v4();

    let mut messages = Vec::new();
    for i in 1..=15 {
        messages.push(ChatMessage::new(
            conv_id,
            MessageRole::User,
            format!("Turn {}", i),
        ));
    }

    let (retained, tokens, evicted) = mgr.fit_working_messages(&messages);

    assert_eq!(retained.len(), 8, "Must retain exactly 8 messages");
    assert_eq!(evicted, 7, "Must evict 7 older messages");
    assert_eq!(retained[0].content, "Turn 8", "Oldest retained must be Turn 8");
    assert_eq!(retained[7].content, "Turn 15", "Newest retained must be Turn 15");
    assert!(tokens <= 2400);
}

#[test]
fn test_restart_resilience_and_compaction_recovery() {
    let (store, db_path) = create_temp_store();
    let conv_id = Uuid::new_v4();
    create_conversation(&store, conv_id);

    // 1. Initial process: Add turns 1..10 and compact
    for i in 1..=10 {
        let u = ChatMessage::new(conv_id, MessageRole::User, format!("Phase 1 Turn {}: port {}", i, 3000 + i));
        store.add_message(&u).unwrap();
        let a = ChatMessage::new(conv_id, MessageRole::Assistant, format!("Phase 1 Reply {}", i));
        store.add_message(&a).unwrap();
    }

    let compactor1 = ContinuousCompactor::new(store.clone());
    let res1 = compactor1.consolidate_if_needed(conv_id).unwrap().expect("Must compact phase 1");
    assert_eq!(res1.episode.turn_end, 10);

    // 2. Simulate process restart: create a new compactor instance from store
    drop(compactor1);

    let compactor2 = ContinuousCompactor::new(store.clone());
    let messages_after_restart = store.list_messages(conv_id).unwrap();
    let state = CompactionState::recover(&store, conv_id, &messages_after_restart).unwrap();
    assert_eq!(state.last_compacted_turn, 10, "Must recover last_compacted_turn = 10 from SQLite");
    assert_eq!(state.uncompacted_turns, 0, "Zero uncompacted turns initially after restart");

    // 3. Add turns 11..20
    for i in 11..=20 {
        let u = ChatMessage::new(conv_id, MessageRole::User, format!("Phase 2 Turn {}: endpoint localhost:{}", i, 8000 + i));
        store.add_message(&u).unwrap();
        let a = ChatMessage::new(conv_id, MessageRole::Assistant, format!("Phase 2 Reply {}", i));
        store.add_message(&a).unwrap();
    }

    // 4. Compact second interval
    let res2 = compactor2.consolidate_if_needed(conv_id).unwrap().expect("Must compact phase 2");
    assert_eq!(res2.episode.turn_start, 11);
    assert_eq!(res2.episode.turn_end, 20);

    // 5. Verify SQLite has both episodes
    let episodes = store.list_episodes(conv_id).unwrap();
    assert_eq!(episodes.len(), 2, "Must contain exactly 2 episodes");
    assert_eq!(episodes[0].turn_start, 1);
    assert_eq!(episodes[0].turn_end, 10);
    assert_eq!(episodes[1].turn_start, 11);
    assert_eq!(episodes[1].turn_end, 20);

    cleanup_temp_store(db_path);
}

#[test]
fn test_multi_interval_30_turns_compaction() {
    let (store, db_path) = create_temp_store();
    let conv_id = Uuid::new_v4();
    create_conversation(&store, conv_id);
    let compactor = ContinuousCompactor::new(store.clone());

    // 30 turns added turn by turn with periodic consolidation evaluation
    for turn in 1..=30 {
        let u = ChatMessage::new(
            conv_id,
            MessageRole::User,
            format!("Turn {}: We decided to configure component_{} at port {}", turn, turn, 7000 + turn),
        );
        store.add_message(&u).unwrap();

        let a = ChatMessage::new(
            conv_id,
            MessageRole::Assistant,
            format!("Confirmed turn {}: component_{} configured.", turn, turn),
        );
        store.add_message(&a).unwrap();

        // Evaluate after each turn (as runtime does in send_message)
        let _ = compactor.consolidate_if_needed(conv_id).unwrap();
    }

    let episodes = store.list_episodes(conv_id).unwrap();
    assert_eq!(episodes.len(), 3, "Must produce exactly 3 episodes for 30 turns");
    assert_eq!(episodes[0].turn_span(), 10);
    assert_eq!(episodes[1].turn_span(), 10);
    assert_eq!(episodes[2].turn_span(), 10);

    // Working buffer window check
    let window_mgr = ContextWindowManager::default();
    let all_messages = store.list_messages(conv_id).unwrap();
    let (working, tokens, evicted) = window_mgr.fit_working_messages(&all_messages);
    assert_eq!(working.len(), 8);
    assert!(tokens <= 2400);
    assert_eq!(evicted, 60 - 8);

    cleanup_temp_store(db_path);
}
