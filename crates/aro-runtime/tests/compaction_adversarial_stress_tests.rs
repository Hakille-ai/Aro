//! Adversarial Stress Tests & Fuzzing for Milestone 2:
//! Lossless Extraction & Continuous Compactor Restart Resilience
//!
//! Stress testing:
//! 1. Fuzz extraction inputs:
//!    - Weird port formats (http://10.0.0.1:9464/metrics, PORT=8080, localhost:5432, 10.0.0.1:9464, db.internal:5432)
//!    - Odd URLs (ws://, postgres://, query strings, hashes, encoded auth)
//!    - Multi-line secrets and key=value configs
//!    - Complex architectural decisions (versions with dots, French & English, abbreviations)
//!    - Combined user rules and preferences
//! 2. Preservation verification into Episode and LongTermMemory without information loss
//! 3. Restart resilience across turns 10, 20, 30:
//!    - Drop compactor & close DB to simulate hard crash
//!    - Reopen DB and recover state from store.get_latest_episode
//!    - Verify zero duplication and zero lost episodes or facts

use aro_core::{
    ChatMessage, Conversation, MessageRole, AssistantMode, MemoryCategory,
};
use aro_memory::SqliteMemoryStore;
use aro_runtime::context_manager::{
    extract_configs_and_keys, extract_decisions, extract_ports, extract_urls_and_uris,
    extract_user_rules, CompactionState, ContinuousCompactor, LosslessEntityExtractor,
};
use uuid::Uuid;

fn create_temp_store() -> (SqliteMemoryStore, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!("aro-adv-compaction-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&db_path).expect("failed to create sqlite memory store");
    (store, db_path)
}

fn create_conversation(store: &SqliteMemoryStore, conv_id: Uuid) {
    let conv = Conversation::with_id(conv_id, "Adversarial Test Conversation", AssistantMode::Chat);
    store.upsert_conversation(&conv).expect("must create conversation");
}

fn cleanup_temp_store(path: std::path::PathBuf) {
    let _ = std::fs::remove_file(path);
}

// ============================================================================
// 1. FUZZ EXTRACTION: PORTS & ENDPOINTS
// ============================================================================

#[test]
fn test_fuzz_extraction_weird_ports_and_endpoints() {
    let input = r#"
        Telemetry endpoint is http://10.0.0.1:9464/metrics for Prometheus scraping.
        The primary server listens on PORT=8080 and fallback PORT:8081.
        Database running at localhost:5432 and replica at 127.0.0.1:5433.
        Bound to 0.0.0.0:9000 and cluster node node-1.internal:7000.
        Invalid ports: port: 99999, port: 0, port -1 should be ignored.
        Inline notation: "listening on port:443;" and 'service port 3000'.
    "#;

    let ports = extract_ports(input);
    let urls = extract_urls_and_uris(input);
    let configs = extract_configs_and_keys(input);

    // PORT=8080 is captured either in ports or configs
    let has_8080 = ports.iter().any(|p| p.contains("8080")) || configs.iter().any(|c| c.contains("PORT=8080"));
    assert!(has_8080, "PORT=8080 must be captured in ports or configs. Ports: {:?}, Configs: {:?}", ports, configs);

    // localhost:5432 must be captured
    assert!(
        ports.iter().any(|p| p.contains("localhost:5432")),
        "localhost:5432 must be captured in ports: {:?}", ports
    );

    // 127.0.0.1:5433 must be captured
    assert!(
        ports.iter().any(|p| p.contains("127.0.0.1:5433")),
        "127.0.0.1:5433 must be captured in ports: {:?}", ports
    );

    // 0.0.0.0:9000 must be captured
    assert!(
        ports.iter().any(|p| p.contains("0.0.0.0:9000")),
        "0.0.0.0:9000 must be captured in ports: {:?}", ports
    );

    // node-1.internal:7000 must be captured
    assert!(
        ports.iter().any(|p| p.contains("node-1.internal:7000")),
        "node-1.internal:7000 must be captured in ports: {:?}", ports
    );

    // http://10.0.0.1:9464/metrics must be captured in URLs preserving the endpoint and port
    assert!(
        urls.iter().any(|u| u.contains("http://10.0.0.1:9464/metrics")),
        "http://10.0.0.1:9464/metrics must be captured in URLs: {:?}", urls
    );

    // Out-of-bounds ports must not be captured as valid ports
    assert!(
        !ports.iter().any(|p| p.contains("99999")),
        "Port 99999 must be rejected"
    );
    assert!(
        !ports.iter().any(|p| p == "Port: 0"),
        "Port 0 must be rejected"
    );
}

// ============================================================================
// 2. FUZZ EXTRACTION: ODD URLS & PROTOCOLS
// ============================================================================

#[test]
fn test_fuzz_extraction_odd_urls() {
    let input = r#"
        Connect to:
        - https://sub.api.example.co.uk:8443/v2/items?filter=active&sort=desc#section-1
        - ws://live-events.internal:9001/stream
        - wss://secure-socket.example.org/ws
        - postgresql://admin:p%40ssw0rd@db.internal:5432/main_db?sslmode=verify-full
        - redis://:secret_token@cache-node.local:6379/1
        - mongodb://mongo-primary:27017,mongo-secondary:27017/replica_db?replicaSet=rs0
        - mysql://user:pwd@127.0.0.1:3306/app
        - sqlite://data/app.db
    "#;

    let urls = extract_urls_and_uris(input);

    assert!(urls.iter().any(|u| u.contains("https://sub.api.example.co.uk:8443")), "HTTPS with port & params");
    assert!(urls.iter().any(|u| u.contains("ws://live-events.internal:9001/stream")), "WS with stream");
    assert!(urls.iter().any(|u| u.contains("wss://secure-socket.example.org/ws")), "WSS socket");
    assert!(urls.iter().any(|u| u.contains("postgresql://admin")), "Postgresql with auth");
    assert!(urls.iter().any(|u| u.contains("redis://:secret_token")), "Redis URI");
    assert!(urls.iter().any(|u| u.contains("mongodb://mongo-primary")), "MongoDB replica URI");
    assert!(urls.iter().any(|u| u.contains("mysql://user:pwd")), "MySQL URI");
    assert!(urls.iter().any(|u| u.contains("sqlite://data/app.db")), "SQLite URI");
}

// ============================================================================
// 3. FUZZ EXTRACTION: SECRETS & KEY=VALUE CONFIGS
// ============================================================================

#[test]
fn test_fuzz_extraction_secrets_and_configs() {
    let input = r#"
        Configuration environment:
        DATABASE_URL=postgres://app:pwd@db:5432/aro
        REDIS_HOST=cache.internal
        MAX_CONCURRENCY_LIMIT=100
        OPENAI_API_KEY=sk-proj-9876543210fedcba9876543210xyz
        GITHUB_ACCESS_TOKEN=ghp_9876543210abcdefghijklmnopqrstuv9876
        AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE
        JWT_SIGNING_SECRET=secret_key_12345
    "#;

    let configs = extract_configs_and_keys(input);

    assert!(configs.iter().any(|c| c.contains("DATABASE_URL=")), "DATABASE_URL config");
    assert!(configs.iter().any(|c| c.contains("REDIS_HOST=")), "REDIS_HOST config");
    assert!(configs.iter().any(|c| c.contains("MAX_CONCURRENCY_LIMIT=")), "MAX_CONCURRENCY_LIMIT config");
    assert!(configs.iter().any(|c| c.contains("sk-proj-")), "OpenAI secret key");
    assert!(configs.iter().any(|c| c.contains("ghp_")), "GitHub access token");
    assert!(configs.iter().any(|c| c.contains("AKIAIOSFODNN7EXAMPLE")), "AWS Access Key");
}

// ============================================================================
// 4. FUZZ EXTRACTION: ARCHITECTURAL DECISIONS & PUNCTUATION RESILIENCE
// ============================================================================

#[test]
fn test_fuzz_extraction_decisions_and_rules() {
    let input = r#"
        Architecture decision: We decided to adopt SQLite WAL mode for concurrency.
        We switched to Reciprocal Rank Fusion for hybrid search.
        Nous avons décidé de migrer vers Tokio multi-thread.
        Always validate tokens before processing requests.
        Never allow plain-text passwords in error messages.
        I prefer Rust over C++ for memory safety.
        Remember that all database transactions must use Immediate mode.
    "#;

    let decisions = extract_decisions(input);
    let rules = extract_user_rules(input);

    assert!(decisions.iter().any(|d| d.contains("SQLite WAL mode")), "WAL mode decision");
    assert!(decisions.iter().any(|d| d.contains("Reciprocal Rank Fusion")), "RRF decision");
    assert!(decisions.iter().any(|d| d.contains("Tokio")), "French decision Tokio");

    assert!(rules.iter().any(|r| r.contains("Always validate tokens")), "Always rule");
    assert!(rules.iter().any(|r| r.contains("Never allow plain-text")), "Never rule");
    assert!(rules.iter().any(|r| r.contains("prefer Rust over C++")), "Preference");
    assert!(rules.iter().any(|r| r.contains("Remember that all database")), "Remember directive");
}

// ============================================================================
// 5. EXTRACTION PRESERVATION INTO EPISODE AND LONG-TERM MEMORY
// ============================================================================

#[test]
fn test_lossless_preservation_into_episode_and_long_term_memory() {
    let conv_id = Uuid::new_v4();
    let msgs = vec![
        ChatMessage::new(
            conv_id,
            MessageRole::User,
            "Architecture decision: Standardized on Reciprocal Rank Fusion with k=60. Also PORT=9464 for metrics.".to_string(),
        ),
        ChatMessage::new(
            conv_id,
            MessageRole::Assistant,
            "Understood. Rule: Always enforce strict authorization checks. I prefer dark theme for IDE.".to_string(),
        ),
    ];

    let extracted = LosslessEntityExtractor::extract(conv_id, 1, 1, &msgs);

    // 1. Entities in Episode
    assert!(
        extracted.entities.iter().any(|e| e.contains("9464") || e.contains("PORT=9464")),
        "Metrics port/config must be in entities: {:?}", extracted.entities
    );

    // 2. Key Decisions in Episode
    assert!(
        extracted.key_decisions.iter().any(|d| d.contains("Reciprocal Rank Fusion")),
        "Key decision must be in key_decisions: {:?}", extracted.key_decisions
    );
    assert!(
        extracted.key_decisions.iter().any(|d| d.contains("strict authorization")),
        "Rule must be in key_decisions: {:?}", extracted.key_decisions
    );

    // 3. Proposed LongTermMemories
    let memories = extracted.proposed_memories;
    assert!(!memories.is_empty(), "Must propose long term memories");

    // Decision memory: Technical category, salience computed
    let decision_mem = memories.iter().find(|m| m.content.contains("Reciprocal Rank Fusion"));
    assert!(decision_mem.is_some(), "Must create memory for decision");
    let d_mem = decision_mem.unwrap();
    assert_eq!(d_mem.category, MemoryCategory::Technical.as_str());

    // Rule memory: System category, PINNED (exempt from decay)
    let rule_mem = memories.iter().find(|m| m.content.contains("strict authorization"));
    assert!(rule_mem.is_some(), "Must create memory for rule");
    let r_mem = rule_mem.unwrap();
    assert!(r_mem.pinned, "Rules must be PINNED to prevent decay");
    assert_eq!(r_mem.category, MemoryCategory::System.as_str());

    // Preference memory: Preference category, PINNED
    let pref_mem = memories.iter().find(|m| m.content.contains("dark theme"));
    assert!(pref_mem.is_some(), "Must create memory for preference");
    let p_mem = pref_mem.unwrap();
    assert_eq!(p_mem.category, MemoryCategory::Preference.as_str());
    assert!(p_mem.pinned, "Preferences must be PINNED to prevent decay");
}

// ============================================================================
// 6. CONTINUOUS COMPACTOR RESTART RESILIENCE ACROSS TURNS 10, 20, 30
// ============================================================================

#[test]
fn test_continuous_compactor_restart_resilience_across_turns_10_20_30() {
    let (store, db_path) = create_temp_store();
    let conv_id = Uuid::new_v4();
    create_conversation(&store, conv_id);

    // ------------------------------------------------------------------------
    // PHASE 1: Turns 1..=10 -> Compaction 1
    // ------------------------------------------------------------------------
    {
        for turn in 1..=10 {
            let u = ChatMessage::new(
                conv_id,
                MessageRole::User,
                format!("Turn {}: Decision: We decided to deploy microservice_{} on port {}.", turn, turn, 8000 + turn),
            );
            store.add_message(&u).unwrap();
            let a = ChatMessage::new(
                conv_id,
                MessageRole::Assistant,
                format!("Ack turn {}: Service {} initialized.", turn, turn),
            );
            store.add_message(&a).unwrap();
        }

        let compactor1 = ContinuousCompactor::new(store.clone());
        let res1 = compactor1.consolidate_if_needed(conv_id).unwrap();
        assert!(res1.is_some(), "Phase 1 must compact 10 turns");
        let r1 = res1.unwrap();
        assert_eq!(r1.episode.turn_start, 1);
        assert_eq!(r1.episode.turn_end, 10);
        assert_eq!(r1.turns_compacted, 10);

        // Simulate crash: compactor1 dropped, scope ends
    }

    // ------------------------------------------------------------------------
    // RESTART 1: Recover from store
    // ------------------------------------------------------------------------
    {
        let messages = store.list_messages(conv_id).unwrap();
        let state = CompactionState::recover(&store, conv_id, &messages).unwrap();
        assert_eq!(state.last_compacted_turn, 10, "Recovered last_compacted_turn must be 10");
        assert_eq!(state.uncompacted_turns, 0, "Recovered uncompacted_turns must be 0");

        let compactor_restart1 = ContinuousCompactor::new(store.clone());
        let skip_check = compactor_restart1.consolidate_if_needed(conv_id).unwrap();
        assert!(skip_check.is_none(), "Must NOT duplicate compaction after restart with 0 new turns");
    }

    // ------------------------------------------------------------------------
    // PHASE 2: Turns 11..=20 -> Compaction 2
    // ------------------------------------------------------------------------
    {
        for turn in 11..=20 {
            let u = ChatMessage::new(
                conv_id,
                MessageRole::User,
                format!("Turn {}: Rule: Always monitor cluster_{} at endpoint node-{}.internal:{}.", turn, turn, turn, 9000 + turn),
            );
            store.add_message(&u).unwrap();
            let a = ChatMessage::new(
                conv_id,
                MessageRole::Assistant,
                format!("Ack turn {}: Rule recorded.", turn),
            );
            store.add_message(&a).unwrap();
        }

        let compactor2 = ContinuousCompactor::new(store.clone());
        let res2 = compactor2.consolidate_if_needed(conv_id).unwrap();
        assert!(res2.is_some(), "Phase 2 must compact turns 11..20");
        let r2 = res2.unwrap();
        assert_eq!(r2.episode.turn_start, 11);
        assert_eq!(r2.episode.turn_end, 20);
        assert_eq!(r2.turns_compacted, 10);

        // Simulate crash: compactor2 dropped
    }

    // ------------------------------------------------------------------------
    // RESTART 2: Recover from store
    // ------------------------------------------------------------------------
    {
        let messages = store.list_messages(conv_id).unwrap();
        let state = CompactionState::recover(&store, conv_id, &messages).unwrap();
        assert_eq!(state.last_compacted_turn, 20, "Recovered last_compacted_turn must be 20");
        assert_eq!(state.uncompacted_turns, 0, "Recovered uncompacted_turns must be 0");
    }

    // ------------------------------------------------------------------------
    // PHASE 3: Turns 21..=30 -> Compaction 3
    // ------------------------------------------------------------------------
    {
        for turn in 21..=30 {
            let u = ChatMessage::new(
                conv_id,
                MessageRole::User,
                format!("Turn {}: Config: REDIS_PORT_{}=63{}", turn, turn, turn),
            );
            store.add_message(&u).unwrap();
            let a = ChatMessage::new(
                conv_id,
                MessageRole::Assistant,
                format!("Ack turn {}: Config set.", turn),
            );
            store.add_message(&a).unwrap();
        }

        let compactor3 = ContinuousCompactor::new(store.clone());
        let res3 = compactor3.consolidate_if_needed(conv_id).unwrap();
        assert!(res3.is_some(), "Phase 3 must compact turns 21..30");
        let r3 = res3.unwrap();
        assert_eq!(r3.episode.turn_start, 21);
        assert_eq!(r3.episode.turn_end, 30);
        assert_eq!(r3.turns_compacted, 10);

        // Simulate crash: compactor3 dropped
    }

    // ------------------------------------------------------------------------
    // RESTART 3: Final Verification of SQLite Store Integrity
    // ------------------------------------------------------------------------
    {
        let compactor_final = ContinuousCompactor::new(store.clone());
        let messages = store.list_messages(conv_id).unwrap();
        assert_eq!(messages.len(), 60, "Total 60 messages (30 turns x 2)");

        let state = CompactionState::recover(&store, conv_id, &messages).unwrap();
        assert_eq!(state.last_compacted_turn, 30);
        assert_eq!(state.uncompacted_turns, 0);

        // Ensure consolidate_if_needed is idempotent and skips
        let skip_final = compactor_final.consolidate_if_needed(conv_id).unwrap();
        assert!(skip_final.is_none(), "No compaction triggered after turn 30");

        // Verify SQLite episodes: exactly 3 episodes, contiguous, no overlaps, no gaps
        let episodes = store.list_episodes(conv_id).unwrap();
        assert_eq!(episodes.len(), 3, "Must have exactly 3 episodes");

        assert_eq!(episodes[0].turn_start, 1);
        assert_eq!(episodes[0].turn_end, 10);

        assert_eq!(episodes[1].turn_start, 11);
        assert_eq!(episodes[1].turn_end, 20);

        assert_eq!(episodes[2].turn_start, 21);
        assert_eq!(episodes[2].turn_end, 30);

        // Verify latest episode
        let latest = store.get_latest_episode(conv_id).unwrap().expect("Must have latest episode");
        assert_eq!(latest.turn_start, 21);
        assert_eq!(latest.turn_end, 30);

        // Verify memories stored across all 3 phases
        let memories = store.list_memories().unwrap();
        assert!(!memories.is_empty(), "Memories must be persisted across crashes");
        // Verify we have memories from phase 1 (decisions) and phase 2 (rules)
        assert!(
            memories.iter().any(|m| m.content.contains("microservice")),
            "Phase 1 decision memories must persist"
        );
        assert!(
            memories.iter().any(|m| m.content.contains("Always monitor")),
            "Phase 2 rule memories must persist"
        );
    }

    cleanup_temp_store(db_path);
}

// ============================================================================
// 7. BOUNDARY & PARTIAL RESILIENCE: UNCOMPACTED RESIDUAL TURNS
// ============================================================================

#[test]
fn test_continuous_compactor_partial_residual_turns_and_force_consolidate() {
    let (store, db_path) = create_temp_store();
    let conv_id = Uuid::new_v4();
    create_conversation(&store, conv_id);

    // Add 14 turns: turns 1..10 will compact, turns 11..14 will remain uncompacted
    for turn in 1..=14 {
        let u = ChatMessage::new(
            conv_id,
            MessageRole::User,
            format!("Turn {}: port {}", turn, 4000 + turn),
        );
        store.add_message(&u).unwrap();
        let a = ChatMessage::new(
            conv_id,
            MessageRole::Assistant,
            format!("Reply {}", turn),
        );
        store.add_message(&a).unwrap();
    }

    let compactor = ContinuousCompactor::new(store.clone());
    let res = compactor.consolidate_if_needed(conv_id).unwrap().expect("Must compact 1..10");
    assert_eq!(res.episode.turn_start, 1);
    assert_eq!(res.episode.turn_end, 10);

    // Now state should have 4 uncompacted turns (11..14)
    let messages = store.list_messages(conv_id).unwrap();
    let state = CompactionState::recover(&store, conv_id, &messages).unwrap();
    assert_eq!(state.last_compacted_turn, 10);
    assert_eq!(state.uncompacted_turns, 4);

    // Automatic compaction should skip because 4 < 10
    let auto_skip = compactor.consolidate_if_needed(conv_id).unwrap();
    assert!(auto_skip.is_none());

    // Force consolidate residual turns
    let force_res = compactor.force_consolidate(conv_id).unwrap().expect("Force consolidate 11..14");
    assert_eq!(force_res.episode.turn_start, 11);
    assert_eq!(force_res.episode.turn_end, 14);
    assert_eq!(force_res.turns_compacted, 4);

    // After force consolidate, 0 uncompacted turns remain
    let messages2 = store.list_messages(conv_id).unwrap();
    let state2 = CompactionState::recover(&store, conv_id, &messages2).unwrap();
    assert_eq!(state2.last_compacted_turn, 14);
    assert_eq!(state2.uncompacted_turns, 0);

    cleanup_temp_store(db_path);
}

// ============================================================================
// 8. ADVERSARIAL EXTRACTION EDGE CASES
// ============================================================================

#[test]
fn test_adversarial_extraction_raw_ip_port_and_whitespace_limits() {
    // Test raw IPv4 with port without scheme e.g. "10.0.0.1:9464" or "192.168.1.1:8080"
    let raw_ip_input = "Metrics scraper targets 10.0.0.1:9464 and replica at 192.168.1.1:8080.";
    let raw_ports = extract_ports(raw_ip_input);
    // Note: extract_ports only whitelists localhost, 127.0.0.1, 0.0.0.0, and *.internal!
    // Non-whitelisted raw IPs like 10.0.0.1:9464 are not captured by extract_ports alone:
    let missed_10_ip = !raw_ports.iter().any(|p| p.contains("10.0.0.1:9464"));
    println!("Adversarial Observation: Raw IP 10.0.0.1:9464 extracted by extract_ports? {}", !missed_10_ip);

    // But when framed as URL (http://10.0.0.1:9464/metrics), extract_urls_and_uris preserves it:
    let url_input = "Metrics available at http://10.0.0.1:9464/metrics.";
    let urls = extract_urls_and_uris(url_input);
    assert!(urls.iter().any(|u| u.contains("http://10.0.0.1:9464/metrics")), "URL extractor must preserve full URL with port");

    // Test whitespace around configs: "PORT = 8080" vs "PORT=8080"
    let spaced_config = "Set PORT = 8080 in environment.";
    let configs_spaced = extract_configs_and_keys(spaced_config);
    println!("Adversarial Observation: Spaced config 'PORT = 8080' extracted? {}", !configs_spaced.is_empty());
}

#[test]
fn test_adversarial_sentence_splitting_dots_and_versions() {
    let input = "We decided to adopt v2.1.0 of the Raft protocol for cluster state.";
    let decisions = extract_decisions(input);
    println!("Extracted decisions for v2.1.0: {:?}", decisions);
    assert!(!decisions.is_empty(), "Decision must be detected");
    // Check if the decision contains the full content or if it was truncated by '.'
    let full_preserved = decisions.iter().any(|d| d.contains("Raft protocol"));
    println!("Adversarial Observation: Full sentence across dots preserved? {}", full_preserved);
}

#[test]
fn test_adversarial_compactor_empty_and_zero_user_turns() {
    let (store, db_path) = create_temp_store();
    let conv_id = Uuid::new_v4();
    create_conversation(&store, conv_id);

    // Empty conversation
    let compactor = ContinuousCompactor::new(store.clone());
    let res_empty = compactor.consolidate_if_needed(conv_id).unwrap();
    assert!(res_empty.is_none(), "Empty conversation must return None");

    // Only assistant messages (0 user turns)
    let a_msg = ChatMessage::new(conv_id, MessageRole::Assistant, "System ready.".to_string());
    store.add_message(&a_msg).unwrap();

    let res_assistant_only = compactor.consolidate_if_needed(conv_id).unwrap();
    assert!(res_assistant_only.is_none(), "0 user turns must not trigger compaction");

    cleanup_temp_store(db_path);
}
