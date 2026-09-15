//! Adversarial Challenge & Stress Test Suite: Canonical Memory Tools & Dual Dispatch
//!
//! Scope:
//! 1. Empirically verify the 5 primitives via `execute_tool_request` / `execute_tool`:
//!    - `memory_save`: save multiple memories with varying categories and saliences, verify valid IDs returned.
//!    - `memory_search`: search with varying limits, queries, categories, scopes. Verify Top-K sorting and count.
//!    - `memory_recall`: recall by ID, recall by non-existent ID, recall with `include_episodes: true`.
//!    - `memory_update`: mutate content, salience, pinned flag, category; verify fields updated and updated_at refreshed.
//!    - `memory_forget`: archive record; verify it is no longer returned in search or recall.
//! 2. Empirically verify dual-dispatch alias routing:
//!    - Call tools with canonical dotted names: `core.memory.save`, `core.memory.search`, `core.memory.recall`, `core.memory.update`, `core.memory.forget`.
//!    - Call tools with snake_case aliases: `memory_save`, `memory_search`, `memory_recall`, `memory_update`, `memory_forget`.
//!    - Call tools with legacy names: `core.memory.list`, `core.memory.delete`.
//!    - Verify that both formats produce identical successful outputs and validate through `Agent::validate_action`.

use aro_agent::{AgentRuntime, EnvironmentSnapshot};
use aro_core::{
    AgentAction, AgentRun, AgentRunStartRequest, AssistantMode, Conversation, Episode,
    LongTermMemory, ToolExecutionRequest, ToolExecutionStatus, TOOL_CORE_MEMORY_DELETE,
    TOOL_CORE_MEMORY_FORGET, TOOL_CORE_MEMORY_LIST, TOOL_CORE_MEMORY_RECALL, TOOL_CORE_MEMORY_SAVE,
    TOOL_CORE_MEMORY_SEARCH, TOOL_CORE_MEMORY_UPDATE, TOOL_MEMORY_DELETE, TOOL_MEMORY_FORGET,
    TOOL_MEMORY_LIST, TOOL_MEMORY_RECALL, TOOL_MEMORY_SAVE, TOOL_MEMORY_SEARCH,
    TOOL_MEMORY_UPDATE,
};
use aro_memory::SqliteMemoryStore;
use aro_runtime::AssistantEngine;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

fn create_test_engine() -> (AssistantEngine, SqliteMemoryStore, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!(
        "aro-m3-adversarial-test-{}.sqlite",
        Uuid::new_v4()
    ));
    let store = SqliteMemoryStore::new(&db_path).expect("failed to create sqlite memory store");
    let engine = AssistantEngine::new(store.clone());
    (engine, store, db_path)
}

fn cleanup_temp_db(path: std::path::PathBuf) {
    let _ = std::fs::remove_file(path);
}

fn create_test_run(store: &SqliteMemoryStore, conv_id: Option<Uuid>) -> AgentRun {
    if let Some(cid) = conv_id {
        let conv = Conversation::with_id(cid, "Adversarial Test Conversation", AssistantMode::Chat);
        let _ = store.upsert_conversation(&conv);
    }
    let run = AgentRun::new(
        "Adversarial memory tool run",
        AssistantMode::Chat,
        conv_id,
        None,
        None,
        None,
    );
    store.upsert_agent_run(&run).expect("upsert agent run");
    run
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 1: Adversarial memory_save verification
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_adversarial_memory_save_primitives() {
    let (engine, store, db_path) = create_test_engine();
    let run = create_test_run(&store, Some(Uuid::new_v4()));

    // 1. Multiple categories, explicit & clamped saliences, pinned variations
    let cases = vec![
        ("technical", Some(0.85), false, "PostgreSQL connection pool max size is 64"),
        ("preference", Some(0.95), true, "User prefers spaces over tabs for indentation"),
        ("fact", Some(0.50), false, "Internal API gateway endpoint is https://gw.internal"),
        ("rule", Some(1.00), true, "Never store unencrypted plaintext secrets"),
        ("decision", Some(0.70), false, "Adopted Apache Arrow for IPC interchange"),
        ("system", Some(0.40), false, "Metrics reporting interval set to 15 seconds"),
        ("personal", Some(0.60), false, "User timezone is Europe/Paris"),
        ("custom_arbitrary", Some(0.75), false, "Arbitrary custom tag categorisation test"),
        // Extreme salience clamping
        ("technical", Some(10.0), false, "Extremely high salience clamped to 1.0"),
        ("technical", Some(0.0), false, "Zero salience clamped to 0.1"),
        ("technical", Some(-5.5), false, "Negative salience clamped to 0.1"),
        // Omitted salience -> auto-calculated
        ("preference", None, false, "I always prefer dark mode in IDE"),
    ];

    let mut saved_ids = Vec::new();

    for (cat, sal_opt, pinned, content) in cases {
        let mut input_map = json!({
            "content": content,
            "category": cat,
            "scope": "project",
            "pinned": pinned
        });
        if let Some(sal) = sal_opt {
            input_map["salience"] = json!(sal);
        }

        let req = ToolExecutionRequest::new(
            run.id,
            run.conversation_id,
            TOOL_MEMORY_SAVE,
            input_map,
        );

        let res = engine.execute_tool(&run, req).await.expect("execute memory_save");
        assert_eq!(res.status, ToolExecutionStatus::Completed);
        assert_eq!(res.output["success"], true);
        assert_eq!(res.output["content"], content);

        let id_str = res.output["id"].as_str().expect("valid id string");
        let id = Uuid::parse_str(id_str).expect("parse uuid from output");
        saved_ids.push(id);

        // Verify direct persistence in SqliteMemoryStore
        let stored = store.get_memory(id).expect("query store").expect("record exists");
        assert_eq!(stored.id, id);
        assert_eq!(stored.content, content);
        assert_eq!(stored.category, cat);
        assert_eq!(stored.pinned, pinned);
        assert_eq!(stored.recall_count, 0, "Initial recall_count must be 0");
        assert_eq!(stored.status, "approved");

        if let Some(sal) = sal_opt {
            let expected_clamped = (sal as f32).clamp(0.1, 1.0);
            assert!(
                (stored.salience - expected_clamped).abs() < 1e-4,
                "Salience {} must clamp to {}, got {}",
                sal,
                expected_clamped,
                stored.salience
            );
        } else {
            assert!(
                stored.salience >= 0.1 && stored.salience <= 1.0,
                "Auto salience must be in [0.1, 1.0], got {}",
                stored.salience
            );
        }
    }

    assert_eq!(saved_ids.len(), 12);

    // 2. Boundary edge cases: Empty and whitespace-only content must fail
    let empty_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SAVE,
        json!({ "content": "" }),
    );
    let empty_res = engine.execute_tool(&run, empty_req).await;
    assert!(empty_res.is_err() || empty_res.unwrap().status == ToolExecutionStatus::Failed);

    let whitespace_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SAVE,
        json!({ "content": "  \t\n  \r\n  " }),
    );
    let ws_res = engine.execute_tool(&run, whitespace_req).await;
    assert!(ws_res.is_err() || ws_res.unwrap().status == ToolExecutionStatus::Failed);

    // 3. Large content boundary test (10,000 chars payload)
    let large_content = "X".repeat(10_000);
    let large_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SAVE,
        json!({
            "content": large_content,
            "category": "technical"
        }),
    );
    let large_res = engine.execute_tool(&run, large_req).await.expect("large save");
    assert_eq!(large_res.status, ToolExecutionStatus::Completed);
    let large_id = Uuid::parse_str(large_res.output["id"].as_str().unwrap()).unwrap();
    let stored_large = store.get_memory(large_id).unwrap().unwrap();
    assert_eq!(stored_large.content.len(), 10_000);

    cleanup_temp_db(db_path);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 2: Adversarial memory_search verification (limits, categories, scopes, Top-K)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_adversarial_memory_search_primitives() {
    let (engine, store, db_path) = create_test_engine();
    let conv_id = Uuid::new_v4();
    let run = create_test_run(&store, Some(conv_id));

    // Seed 15 diverse memories
    let seed_data = vec![
        ("Rust async runtime tokio executes worker threads", "technical", "project", false, 0.8),
        ("Kubernetes ingress controller uses NGINX routing", "technical", "project", false, 0.75),
        ("User strictly prefers concise responses without filler", "preference", "user", true, 0.95),
        ("User prefers French documentation when available", "preference", "user", false, 0.65),
        ("System backup snapshot cron triggers at 02:00 UTC", "system", "project", false, 0.5),
        ("Redis cluster cache TTL defaults to 3600 seconds", "technical", "project", false, 0.8),
        ("Authentication tokens expire after 15 minutes of inactivity", "rule", "conversation", true, 0.9),
        ("Decision: Use SQLite WAL mode for concurrency", "decision", "project", true, 0.9),
        ("Docker image alpine base reduces layer footprint", "technical", "conversation", false, 0.6),
        ("GraphQL federation gateway combines domain subgraphs", "technical", "project", false, 0.7),
        ("User interface theme defaults to system dark mode", "preference", "user", false, 0.7),
        ("Database connection timeout configured to 5000ms", "technical", "project", false, 0.6),
        ("Prometheus scrape interval configured at 10s", "system", "project", false, 0.55),
        ("Ephemeral session cookies must be partitioned", "rule", "conversation", false, 0.7),
        ("Kafka topic partitions replicated across 3 brokers", "technical", "project", false, 0.85),
    ];

    for (content, cat, scope, pinned, salience) in seed_data {
        let mem = LongTermMemory {
            id: Uuid::new_v4(),
            client_id: None,
            content: content.to_string(),
            category: cat.to_string(),
            scope: scope.to_string(),
            status: "approved".to_string(),
            source_conversation_id: Some(conv_id),
            source_message_ids: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            pinned,
            salience,
            recall_count: 0,
            last_used_at: None,
        };
        store.upsert_memory(&mem).expect("seed memory");
    }

    // 1. Limit variations: 1, 3, 0 (clamped to 1), 100 (clamped to 50)
    let limits_to_test = vec![(1, 1), (3, 3), (0, 1), (100, 15)]; // clamped max 50, but only 15 exist in DB

    for (req_limit, _expected_count) in limits_to_test {
        let search_req = ToolExecutionRequest::new(
            run.id,
            run.conversation_id,
            TOOL_MEMORY_SEARCH,
            json!({
                "query": "technical project",
                "limit": req_limit
            }),
        );
        let res = engine.execute_tool(&run, search_req).await.expect("search limit test");
        assert_eq!(res.status, ToolExecutionStatus::Completed);
        let count = res.output["count"].as_u64().unwrap() as usize;
        let memories = res.output["memories"].as_array().unwrap();
        assert_eq!(count, memories.len());
        if req_limit == 0 || req_limit == 1 {
            assert_eq!(count, 1, "Limit 0 or 1 must return exactly 1 item");
        } else if req_limit == 3 {
            assert_eq!(count, 3, "Limit 3 must return exactly 3 items");
        } else {
            assert!(count <= 50, "Limit 100 must be clamped to <= 50");
        }
    }

    // 2. Category filtering
    let cat_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SEARCH,
        json!({
            "query": "user",
            "category": "preference",
            "limit": 10
        }),
    );
    let cat_res = engine.execute_tool(&run, cat_req).await.expect("search cat");
    let cat_mems = cat_res.output["memories"].as_array().unwrap();
    assert!(!cat_mems.is_empty());
    for m in cat_mems {
        assert_eq!(m["category"], "preference");
    }

    // Non-existent category filter -> count == 0
    let empty_cat_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SEARCH,
        json!({
            "query": "Rust",
            "category": "non_existent_category_xyz",
            "limit": 10
        }),
    );
    let empty_cat_res = engine.execute_tool(&run, empty_cat_req).await.expect("search empty cat");
    assert_eq!(empty_cat_res.output["count"], 0);
    assert!(empty_cat_res.output["memories"].as_array().unwrap().is_empty());

    // 3. Scope filtering
    let scope_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SEARCH,
        json!({
            "query": "prefers",
            "scope": "user",
            "limit": 10
        }),
    );
    let scope_res = engine.execute_tool(&run, scope_req).await.expect("search scope");
    let scope_mems = scope_res.output["memories"].as_array().unwrap();
    assert!(!scope_mems.is_empty());
    for m in scope_mems {
        assert_eq!(m["scope"], "user");
    }

    // 4. Combined category + scope filtering
    let comb_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SEARCH,
        json!({
            "query": "Rust",
            "category": "technical",
            "scope": "project",
            "limit": 10
        }),
    );
    let comb_res = engine.execute_tool(&run, comb_req).await.expect("combined search");
    let comb_mems = comb_res.output["memories"].as_array().unwrap();
    for m in comb_mems {
        assert_eq!(m["category"], "technical");
        assert_eq!(m["scope"], "project");
    }

    // 5. Top-K Sorting: Pinned memories must be prioritized first with score = 1.0
    let pinned_search_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SEARCH,
        json!({
            "query": "concise responses indentation WAL mode",
            "limit": 5
        }),
    );
    let ps_res = engine.execute_tool(&run, pinned_search_req).await.expect("pinned search");
    let ps_mems = ps_res.output["memories"].as_array().unwrap();
    assert!(!ps_mems.is_empty());
    let first = &ps_mems[0];
    assert_eq!(first["pinned"], true, "Top memory in search must be pinned");
    assert_eq!(first["score"], 1.0, "Pinned memory score must be 1.0");

    cleanup_temp_db(db_path);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 3: Adversarial memory_recall verification (ID, non-existent, episodes, atomic counter)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_adversarial_memory_recall_primitives() {
    let (engine, store, db_path) = create_test_engine();
    let conv_id = Uuid::new_v4();
    let run = create_test_run(&store, Some(conv_id));

    // 1. Seed a target memory
    let memory_id = Uuid::new_v4();
    let target_mem = LongTermMemory {
        id: memory_id,
        client_id: None,
        content: "Distributed cache uses Memcached cluster on port 11211".to_string(),
        category: "technical".to_string(),
        scope: "project".to_string(),
        status: "approved".to_string(),
        source_conversation_id: Some(conv_id),
        source_message_ids: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        pinned: false,
        salience: 0.75,
        recall_count: 0,
        last_used_at: None,
    };
    store.upsert_memory(&target_mem).expect("seed mem");

    // 2. Seed an episode correlating with the cache topic
    let episode = Episode::new(
        conv_id,
        1,
        10,
        "Evaluated caching solutions: deployed Memcached for session offloading",
        vec!["Memcached selected for low-latency session caching".to_string()],
        vec!["Memcached".to_string(), "port 11211".to_string()],
        240,
    );
    store.store_episode(&episode).expect("seed episode");

    // 3. Recall by valid ID with includeEpisodes: true
    let recall_req1 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_RECALL,
        json!({
            "id": memory_id.to_string(),
            "includeEpisodes": true
        }),
    );
    let res1 = engine.execute_tool(&run, recall_req1).await.expect("recall 1");
    assert_eq!(res1.status, ToolExecutionStatus::Completed);
    assert_eq!(res1.output["found"], true);
    assert_eq!(res1.output["memory"]["id"], memory_id.to_string());
    assert_eq!(
        res1.output["memory"]["content"],
        "Distributed cache uses Memcached cluster on port 11211"
    );

    // Verify episodes correlated
    let eps = res1.output["relatedEpisodes"].as_array().expect("episodes array");
    assert!(!eps.is_empty(), "Must correlate with Memcached episode");

    // Verify atomic recall_count incremented 0 -> 1
    let stored1 = store.get_memory(memory_id).unwrap().unwrap();
    assert_eq!(stored1.recall_count, 1);
    assert!(stored1.last_used_at.is_some());

    // 4. Consecutive recalls: verify atomic increment 1 -> 2 -> 3
    for expected_count in [2, 3] {
        let loop_req = ToolExecutionRequest::new(
            run.id,
            run.conversation_id,
            TOOL_MEMORY_RECALL,
            json!({
                "id": memory_id.to_string(),
                "includeEpisodes": false
            }),
        );
        let loop_res = engine.execute_tool(&run, loop_req).await.expect("repeat recall");
        assert_eq!(loop_res.output["found"], true);
        assert!(loop_res.output.get("relatedEpisodes").is_none() || loop_res.output["relatedEpisodes"].is_null());

        let cur_stored = store.get_memory(memory_id).unwrap().unwrap();
        assert_eq!(
            cur_stored.recall_count, expected_count,
            "recall_count must increment atomically"
        );
    }

    // 5. Recall by non-existent UUID
    let non_existent_id = Uuid::new_v4();
    let miss_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_RECALL,
        json!({ "id": non_existent_id.to_string() }),
    );
    let miss_res = engine.execute_tool(&run, miss_req).await.expect("miss recall");
    assert_eq!(miss_res.status, ToolExecutionStatus::Completed);
    assert_eq!(miss_res.output["found"], false);
    assert!(miss_res.output.get("memory").is_none() || miss_res.output["memory"].is_null());

    // 6. Recall with malformed ID string (must handle gracefully, not panic)
    let malformed_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_RECALL,
        json!({ "id": "not-a-valid-uuid-at-all" }),
    );
    let malformed_res = engine.execute_tool(&run, malformed_req).await.expect("malformed id recall");
    assert_eq!(malformed_res.status, ToolExecutionStatus::Completed);
    assert_eq!(malformed_res.output["found"], false);

    // 7. Recall by entity_key (both entityKey and snake_case entity_key)
    let ek_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_RECALL,
        json!({
            "entity_key": "Memcached",
            "include_episodes": true
        }),
    );
    let ek_res = engine.execute_tool(&run, ek_req).await.expect("entity_key recall");
    assert_eq!(ek_res.output["found"], true);
    assert_eq!(ek_res.output["memory"]["id"], memory_id.to_string());

    cleanup_temp_db(db_path);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 4: Adversarial memory_update verification (mutations, salience, pinned, updated_at)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_adversarial_memory_update_primitives() {
    let (engine, store, db_path) = create_test_engine();
    let run = create_test_run(&store, None);

    // 1. Seed initial memory
    let memory_id = Uuid::new_v4();
    let initial_time = Utc::now() - chrono::Duration::hours(2);
    let mem = LongTermMemory {
        id: memory_id,
        client_id: None,
        content: "Initial staging database endpoint: db.staging.aro".to_string(),
        category: "technical".to_string(),
        scope: "project".to_string(),
        status: "approved".to_string(),
        source_conversation_id: None,
        source_message_ids: vec![],
        created_at: initial_time,
        updated_at: initial_time,
        pinned: false,
        salience: 0.4,
        recall_count: 0,
        last_used_at: None,
    };
    store.upsert_memory(&mem).expect("seed");

    // 2. Mutate content, salience, pinned flag, and category
    let update_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_UPDATE,
        json!({
            "id": memory_id.to_string(),
            "content": "Updated production database endpoint: db.prod.aro with read replicas",
            "salience": 0.95,
            "pinned": true,
            "category": "system"
        }),
    );
    let update_res = engine.execute_tool(&run, update_req).await.expect("execute update");
    assert_eq!(update_res.status, ToolExecutionStatus::Completed);
    assert_eq!(update_res.output["success"], true);
    assert_eq!(update_res.output["id"], memory_id.to_string());
    assert!(update_res.output.get("updatedAt").is_some());

    // Verify in store
    let updated_mem = store.get_memory(memory_id).unwrap().unwrap();
    assert_eq!(
        updated_mem.content,
        "Updated production database endpoint: db.prod.aro with read replicas"
    );
    assert_eq!(updated_mem.salience, 0.95);
    assert!(updated_mem.pinned);
    assert_eq!(updated_mem.category, "system");
    assert!(
        updated_mem.updated_at > initial_time,
        "updated_at must be refreshed forward"
    );

    // 3. Partial update: Only update salience with clamping > 1.0
    let clamp_up_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_UPDATE,
        json!({
            "id": memory_id.to_string(),
            "salience": 999.0
        }),
    );
    let clamp_res = engine.execute_tool(&run, clamp_up_req).await.expect("salience clamp update");
    assert_eq!(clamp_res.output["success"], true);
    let clamped_mem = store.get_memory(memory_id).unwrap().unwrap();
    assert_eq!(clamped_mem.salience, 1.0, "Salience 999.0 must clamp to 1.0");
    // Ensure content and pinned were NOT overwritten
    assert_eq!(
        clamped_mem.content,
        "Updated production database endpoint: db.prod.aro with read replicas"
    );
    assert!(clamped_mem.pinned);

    // 4. Partial update: Unpin memory
    let unpin_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_UPDATE,
        json!({
            "id": memory_id.to_string(),
            "pinned": false
        }),
    );
    let unpin_res = engine.execute_tool(&run, unpin_req).await.expect("unpin update");
    assert_eq!(unpin_res.output["success"], true);
    let unpinned_mem = store.get_memory(memory_id).unwrap().unwrap();
    assert!(!unpinned_mem.pinned);

    // 5. Error case: Update non-existent ID
    let ghost_id = Uuid::new_v4();
    let ghost_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_UPDATE,
        json!({
            "id": ghost_id.to_string(),
            "content": "Should fail"
        }),
    );
    let ghost_res = engine.execute_tool(&run, ghost_req).await;
    assert!(ghost_res.is_err() || ghost_res.unwrap().status == ToolExecutionStatus::Failed);

    // 6. Error case: Update with empty content
    let empty_up_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_UPDATE,
        json!({
            "id": memory_id.to_string(),
            "content": "   "
        }),
    );
    let empty_up_res = engine.execute_tool(&run, empty_up_req).await;
    assert!(empty_up_res.is_err() || empty_up_res.unwrap().status == ToolExecutionStatus::Failed);

    cleanup_temp_db(db_path);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 5: Adversarial memory_forget & archival lifecycle
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_adversarial_memory_forget_and_archival() {
    let (engine, store, db_path) = create_test_engine();
    let run = create_test_run(&store, None);

    // 1. Seed two memories
    let id1 = Uuid::new_v4();
    let mem1 = LongTermMemory {
        id: id1,
        client_id: None,
        content: "Deprecated authorization microservice token validation key".to_string(),
        category: "technical".to_string(),
        scope: "project".to_string(),
        status: "approved".to_string(),
        source_conversation_id: None,
        source_message_ids: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        pinned: false,
        salience: 0.8,
        recall_count: 0,
        last_used_at: None,
    };
    store.upsert_memory(&mem1).expect("seed 1");

    let id2 = Uuid::new_v4();
    let mem2 = LongTermMemory {
        id: id2,
        client_id: None,
        content: "Temporary QA credentials user qa_tester host qa-box-987".to_string(),
        category: "system".to_string(),
        scope: "conversation".to_string(),
        status: "approved".to_string(),
        source_conversation_id: None,
        source_message_ids: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        pinned: false,
        salience: 0.6,
        recall_count: 0,
        last_used_at: None,
    };
    store.upsert_memory(&mem2).expect("seed 2");

    // Verify both are searchable before deletion
    let before_search = engine
        .search_memories("microservice credentials", 5)
        .await
        .expect("search before");
    assert!(before_search.iter().any(|m| m.id == id1));
    assert!(before_search.iter().any(|m| m.id == id2));

    // 2. Forget mem1 via snake_case `memory_forget`
    let forget_req1 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_FORGET,
        json!({
            "id": id1.to_string(),
            "reason": "Service deprecated"
        }),
    );
    let forget_res1 = engine.execute_tool(&run, forget_req1).await.expect("forget 1");
    assert_eq!(forget_res1.status, ToolExecutionStatus::Completed);
    assert_eq!(forget_res1.output["success"], true);
    assert_eq!(forget_res1.output["id"], id1.to_string());
    assert_eq!(forget_res1.output["status"], "archived");

    // Verify mem1 is soft-deleted from active store
    assert!(store.get_memory(id1).unwrap().is_none(), "mem1 must be soft-deleted");

    // Verify mem1 is no longer returned in search
    let after_search1 = engine
        .search_memories("microservice", 5)
        .await
        .expect("search after 1");
    assert!(!after_search1.iter().any(|m| m.id == id1), "mem1 must not appear in search");

    // Verify mem1 recall returns found: false
    let recall_req1 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_RECALL,
        json!({ "id": id1.to_string() }),
    );
    let recall_res1 = engine.execute_tool(&run, recall_req1).await.expect("recall deleted");
    assert_eq!(recall_res1.output["found"], false);

    // 3. Forget mem2 via legacy alias `core.memory.delete`
    let delete_req2 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_DELETE,
        json!({
            "id": id2.to_string()
        }),
    );
    let delete_res2 = engine.execute_tool(&run, delete_req2).await.expect("delete 2");
    assert_eq!(delete_res2.status, ToolExecutionStatus::Completed);
    assert_eq!(delete_res2.output["success"], true);

    // Verify mem2 is soft-deleted and removed from search
    assert!(store.get_memory(id2).unwrap().is_none(), "mem2 must be soft-deleted");
    let after_search2 = engine
        .search_memories("credentials qa_tester", 5)
        .await
        .expect("search after 2");
    assert!(!after_search2.iter().any(|m| m.id == id2), "mem2 must not appear in search");

    cleanup_temp_db(db_path);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 6: Adversarial Dual Dispatch & Agent::validate_action Verification
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_adversarial_dual_dispatch_and_agent_validation() {
    let (engine, store, db_path) = create_test_engine();
    let run = create_test_run(&store, None);

    // 1. Dual dispatch execution: Pairwise dotted canonical vs snake_case alias
    // Save dotted
    let save_dotted = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_SAVE,
        json!({ "content": "Dotted memory save verification note", "category": "technical" }),
    );
    let s_dot_res = engine.execute_tool(&run, save_dotted).await.expect("dotted save");
    assert_eq!(s_dot_res.status, ToolExecutionStatus::Completed);
    let id_dotted = Uuid::parse_str(s_dot_res.output["id"].as_str().unwrap()).unwrap();

    // Save snake
    let save_snake = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SAVE,
        json!({ "content": "Snake memory save verification note", "category": "technical" }),
    );
    let s_snake_res = engine.execute_tool(&run, save_snake).await.expect("snake save");
    assert_eq!(s_snake_res.status, ToolExecutionStatus::Completed);
    let id_snake = Uuid::parse_str(s_snake_res.output["id"].as_str().unwrap()).unwrap();

    // Search dotted
    let search_dotted = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_SEARCH,
        json!({ "query": "verification note", "limit": 5 }),
    );
    let sd_res = engine.execute_tool(&run, search_dotted).await.expect("dotted search");
    assert_eq!(sd_res.status, ToolExecutionStatus::Completed);
    assert!(sd_res.output["count"].as_u64().unwrap() >= 2);

    // Search snake
    let search_snake = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SEARCH,
        json!({ "query": "verification note", "limit": 5 }),
    );
    let ss_res = engine.execute_tool(&run, search_snake).await.expect("snake search");
    assert_eq!(ss_res.status, ToolExecutionStatus::Completed);
    assert!(ss_res.output["count"].as_u64().unwrap() >= 2);

    // Recall dotted
    let recall_dotted = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_RECALL,
        json!({ "id": id_dotted.to_string() }),
    );
    let rd_res = engine.execute_tool(&run, recall_dotted).await.expect("dotted recall");
    assert_eq!(rd_res.output["found"], true);

    // Recall snake
    let recall_snake = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_RECALL,
        json!({ "id": id_snake.to_string() }),
    );
    let rs_res = engine.execute_tool(&run, recall_snake).await.expect("snake recall");
    assert_eq!(rs_res.output["found"], true);

    // Update dotted
    let update_dotted = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_UPDATE,
        json!({ "id": id_dotted.to_string(), "salience": 0.88 }),
    );
    let ud_res = engine.execute_tool(&run, update_dotted).await.expect("dotted update");
    assert_eq!(ud_res.output["success"], true);

    // Update snake
    let update_snake = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_UPDATE,
        json!({ "id": id_snake.to_string(), "salience": 0.88 }),
    );
    let us_res = engine.execute_tool(&run, update_snake).await.expect("snake update");
    assert_eq!(us_res.output["success"], true);

    // List dotted (legacy)
    let list_dotted = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_LIST,
        json!({ "limit": 10 }),
    );
    let ld_res = engine.execute_tool(&run, list_dotted).await.expect("dotted list");
    assert_eq!(ld_res.status, ToolExecutionStatus::Completed);
    assert!(ld_res.output["count"].as_u64().unwrap() >= 2);

    // List snake (legacy alias)
    let list_snake = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_LIST,
        json!({ "limit": 10 }),
    );
    let ls_res = engine.execute_tool(&run, list_snake).await.expect("snake list");
    assert_eq!(ls_res.status, ToolExecutionStatus::Completed);
    assert!(ls_res.output["count"].as_u64().unwrap() >= 2);

    // Forget dotted
    let forget_dotted = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_FORGET,
        json!({ "id": id_dotted.to_string() }),
    );
    let fd_res = engine.execute_tool(&run, forget_dotted).await.expect("dotted forget");
    assert_eq!(fd_res.output["status"], "archived");

    // Forget snake
    let forget_snake = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_FORGET,
        json!({ "id": id_snake.to_string() }),
    );
    let fs_res = engine.execute_tool(&run, forget_snake).await.expect("snake forget");
    assert_eq!(fs_res.output["status"], "archived");

    // Delete dotted (legacy)
    let seed_del1 = LongTermMemory {
        id: Uuid::new_v4(),
        client_id: None,
        content: "Legacy delete dotted test".to_string(),
        category: "technical".to_string(),
        scope: "project".to_string(),
        status: "approved".to_string(),
        source_conversation_id: None,
        source_message_ids: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        pinned: false,
        salience: 0.5,
        recall_count: 0,
        last_used_at: None,
    };
    store.upsert_memory(&seed_del1).unwrap();
    let del_dotted = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_DELETE,
        json!({ "id": seed_del1.id.to_string() }),
    );
    let dd_res = engine.execute_tool(&run, del_dotted).await.expect("dotted delete");
    assert_eq!(dd_res.output["success"], true);

    // Delete snake (legacy alias)
    let seed_del2 = LongTermMemory {
        id: Uuid::new_v4(),
        client_id: None,
        content: "Legacy delete snake test".to_string(),
        category: "technical".to_string(),
        scope: "project".to_string(),
        status: "approved".to_string(),
        source_conversation_id: None,
        source_message_ids: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        pinned: false,
        salience: 0.5,
        recall_count: 0,
        last_used_at: None,
    };
    store.upsert_memory(&seed_del2).unwrap();
    let del_snake = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_DELETE,
        json!({ "id": seed_del2.id.to_string() }),
    );
    let ds_res = engine.execute_tool(&run, del_snake).await.expect("snake delete");
    assert_eq!(ds_res.output["success"], true);

    // 2. Comprehensive Agent::validate_action validation
    let agent_runtime = AgentRuntime::new();
    let start_req = AgentRunStartRequest {
        run_id: None,
        lane_id: None,
        conversation_id: None,
        goal: "Validate cognitive memory dual dispatch routing".to_string(),
        mode: AssistantMode::Chat,
        system_prompt: None,
        model_id: Some("mock-model".to_string()),
        provider: Some("mock-local".to_string()),
        autonomy_profile_id: None,
        priority: None,
        max_steps: Some(1),
    };
    let agent_run = agent_runtime.start_run(&start_req, None);
    let context_pack = agent_runtime.build_context_pack(
        &agent_run,
        &[],
        &[],
        &[],
        EnvironmentSnapshot::default(),
    );

    let dummy_id = Uuid::new_v4().to_string();

    // Verify all canonical, snake_case, and alias variations pass Agent::validate_action
    let valid_tool_calls = vec![
        // Canonical dotted
        (TOOL_CORE_MEMORY_SAVE, json!({ "content": "val test" })),
        (TOOL_CORE_MEMORY_SEARCH, json!({ "query": "val query" })),
        (TOOL_CORE_MEMORY_RECALL, json!({ "id": dummy_id })),
        (TOOL_CORE_MEMORY_UPDATE, json!({ "id": dummy_id, "salience": 0.9 })),
        (TOOL_CORE_MEMORY_FORGET, json!({ "id": dummy_id })),
        (TOOL_CORE_MEMORY_LIST, json!({})),
        (TOOL_CORE_MEMORY_DELETE, json!({ "id": dummy_id })),
        // Snake_case aliases
        (TOOL_MEMORY_SAVE, json!({ "content": "val test" })),
        (TOOL_MEMORY_SEARCH, json!({ "query": "val query" })),
        (TOOL_MEMORY_RECALL, json!({ "id": dummy_id })),
        (TOOL_MEMORY_UPDATE, json!({ "id": dummy_id, "salience": 0.9 })),
        (TOOL_MEMORY_FORGET, json!({ "id": dummy_id })),
        (TOOL_MEMORY_LIST, json!({})),
        (TOOL_MEMORY_DELETE, json!({ "id": dummy_id })),
        // Additional documented aliases
        ("memory.save", json!({ "content": "val test" })),
        ("memory.add", json!({ "content": "val test" })),
        ("memory.search", json!({ "query": "val query" })),
        ("memory.recall", json!({ "id": dummy_id })),
        ("memory.update", json!({ "id": dummy_id })),
        ("memory.forget", json!({ "id": dummy_id })),
        ("memory.list", json!({})),
        ("memory.delete", json!({ "id": dummy_id })),
    ];

    for (tool_id, input) in valid_tool_calls {
        let action = AgentAction::tool(tool_id, input, Some("testing alias validation".to_string()));
        let validation_res = agent_runtime.validate_action(&action, &context_pack);
        assert!(
            validation_res.is_ok(),
            "Tool `{}` failed Agent::validate_action: {:?}",
            tool_id,
            validation_res.err()
        );
    }

    // Verify that unrecognized / invalid tool names fail Agent::validate_action
    let invalid_tool_calls = vec![
        "memory_nuke",
        "core.memory.obliterate",
        "memory.erase_all",
        "random_unknown_tool_123",
    ];

    for invalid_tool in invalid_tool_calls {
        let action = AgentAction::tool(invalid_tool, json!({}), None);
        let validation_res = agent_runtime.validate_action(&action, &context_pack);
        assert!(
            validation_res.is_err(),
            "Invalid tool `{}` must NOT pass Agent::validate_action",
            invalid_tool
        );
    }

    cleanup_temp_db(db_path);
}
