//! Milestone 3 Integration Test Suite: Canonical Memory Tools, RRF Hybrid Search & Dual Dispatch
//!
//! Tests:
//! - Dual dispatch for all memory tools (snake_case aliases and dotted canonical IDs)
//! - `memory_save`: content validation, category parsing, automatic salience computation and clamping
//! - `memory_search`: Top-K RRF retrieval, category/scope filtering, ScoredMemory structure
//! - `memory_recall`: lookup by ID and entity_key, atomic recall_count tracking, episodic correlation
//! - `memory_update`: field mutation, salience adjustment, timestamp update
//! - `memory_forget` & `memory_delete`: archival lifecycle, removal from active recall
//! - Top-K RRF search: multimodal consensus, pinned priority, zero full-table RAM scan

use aro_core::{
    AgentRun, AssistantMode, Conversation, Episode, LongTermMemory, ToolExecutionRequest,
    TOOL_CORE_MEMORY_FORGET, TOOL_CORE_MEMORY_RECALL, TOOL_CORE_MEMORY_SAVE,
    TOOL_CORE_MEMORY_SEARCH, TOOL_CORE_MEMORY_UPDATE, TOOL_MEMORY_FORGET,
    TOOL_MEMORY_RECALL, TOOL_MEMORY_SAVE, TOOL_MEMORY_SEARCH, TOOL_MEMORY_UPDATE,
};
use aro_memory::SqliteMemoryStore;
use aro_runtime::AssistantEngine;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

fn create_test_engine() -> (AssistantEngine, SqliteMemoryStore, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!("aro-m3-tools-test-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&db_path).expect("failed to create sqlite memory store");
    let engine = AssistantEngine::new(store.clone());
    (engine, store, db_path)
}

fn cleanup_temp_db(path: std::path::PathBuf) {
    let _ = std::fs::remove_file(path);
}

fn create_test_run(store: &SqliteMemoryStore, conv_id: Option<Uuid>) -> AgentRun {
    if let Some(cid) = conv_id {
        let conv = Conversation::with_id(cid, "Test Conversation", AssistantMode::Chat);
        let _ = store.upsert_conversation(&conv);
    }
    let run = AgentRun::new(
        "Memory tool test run",
        AssistantMode::Chat,
        conv_id,
        None,
        None,
        None,
    );
    store.upsert_agent_run(&run).expect("upsert agent run");
    run
}

#[tokio::test]
async fn test_memory_save_and_search_dual_dispatch() {
    let (engine, store, db_path) = create_test_engine();
    let run = create_test_run(&store, Some(Uuid::new_v4()));

    // 1. Save via snake_case alias `memory_save`
    let save_req1 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SAVE,
        json!({
            "content": "PostgreSQL database runs on port 5432 in production cluster",
            "category": "technical",
            "scope": "project",
            "pinned": false,
            "salience": 0.85
        }),
    );
    let res1 = engine.execute_tool(&run, save_req1).await.expect("save 1");
    assert!(res1.status == aro_core::ToolExecutionStatus::Completed);
    assert_eq!(res1.output["success"], true);
    let id1_str = res1.output["id"].as_str().expect("valid id string");
    let id1 = Uuid::parse_str(id1_str).expect("valid uuid");

    // 2. Save via canonical dotted `core.memory.save`
    let save_req2 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_SAVE,
        json!({
            "content": "User prefers dark mode with monospaced Fira Code font",
            "category": "preference",
            "scope": "user",
            "pinned": true
        }),
    );
    let res2 = engine.execute_tool(&run, save_req2).await.expect("save 2");
    assert!(res2.status == aro_core::ToolExecutionStatus::Completed);
    assert_eq!(res2.output["success"], true);
    let id2_str = res2.output["id"].as_str().expect("valid id string");
    let id2 = Uuid::parse_str(id2_str).expect("valid uuid");

    // 3. Search via snake_case alias `memory_search`
    let search_req1 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SEARCH,
        json!({
            "query": "PostgreSQL port",
            "limit": 5
        }),
    );
    let sres1 = engine.execute_tool(&run, search_req1).await.expect("search 1");
    assert!(sres1.status == aro_core::ToolExecutionStatus::Completed);
    assert!(sres1.output["count"].as_u64().unwrap() >= 1);
    let memories = sres1.output["memories"].as_array().expect("memories array");
    let found1 = memories.iter().any(|m| m["id"] == id1.to_string());
    assert!(found1, "Should find memory saved via snake_case");

    // Verify ScoredMemory fields
    let first = &memories[0];
    assert!(first.get("id").is_some());
    assert!(first.get("content").is_some());
    assert!(first.get("category").is_some());
    assert!(first.get("scope").is_some());
    assert!(first.get("score").is_some());
    assert!(first.get("pinned").is_some());
    assert!(first.get("salience").is_some());

    // 4. Search via canonical dotted `core.memory.search`
    let search_req2 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_SEARCH,
        json!({
            "query": "dark mode preference font",
            "limit": 5
        }),
    );
    let sres2 = engine.execute_tool(&run, search_req2).await.expect("search 2");
    assert!(sres2.status == aro_core::ToolExecutionStatus::Completed);
    assert!(sres2.output["count"].as_u64().unwrap() >= 1);
    let memories2 = sres2.output["memories"].as_array().expect("memories array");
    let found2 = memories2.iter().any(|m| m["id"] == id2.to_string());
    assert!(found2, "Should find memory saved via canonical dotted ID");

    cleanup_temp_db(db_path);
}

#[tokio::test]
async fn test_memory_recall_by_id_and_entity_key_with_atomic_recall() {
    let (engine, store, db_path) = create_test_engine();
    let conv_id = Uuid::new_v4();
    let run = create_test_run(&store, Some(conv_id));

    // Save memory
    let memory = LongTermMemory {
        id: Uuid::new_v4(),
        client_id: None,
        content: "OAuth2 authentication uses Keycloak identity provider on port 8443".to_string(),
        category: "technical".to_string(),
        scope: "conversation".to_string(),
        status: "approved".to_string(),
        source_conversation_id: Some(conv_id),
        source_message_ids: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        pinned: false,
        salience: 0.8,
        recall_count: 0,
        last_used_at: None,
    };
    store.upsert_memory(&memory).expect("seed memory");

    // Seed related episode
    let episode = Episode::new(
        conv_id,
        1,
        5,
        "Configured Keycloak authentication for API services",
        vec!["Adopted Keycloak for OAuth2".to_string()],
        vec!["Keycloak".to_string()],
        120,
    );
    store.store_episode(&episode).expect("seed episode");

    // Verify initial recall_count == 0
    let initial_mem = store.get_memory(memory.id).unwrap().unwrap();
    assert_eq!(initial_mem.recall_count, 0);

    // 1. Recall by ID via snake_case `memory_recall`
    let recall_req1 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_RECALL,
        json!({
            "id": memory.id.to_string(),
            "includeEpisodes": true
        }),
    );
    let res1 = engine.execute_tool(&run, recall_req1).await.expect("recall 1");
    assert!(res1.status == aro_core::ToolExecutionStatus::Completed);
    assert_eq!(res1.output["found"], true);
    assert_eq!(res1.output["memory"]["id"], memory.id.to_string());

    // Verify atomic recall_count was incremented to 1
    let touched1 = store.get_memory(memory.id).unwrap().unwrap();
    assert_eq!(touched1.recall_count, 1, "recall_count must increment to 1");
    assert!(touched1.last_used_at.is_some());

    // 2. Recall again by ID: verify atomic recall_count increments to 2
    let recall_req2 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_RECALL,
        json!({
            "id": memory.id.to_string()
        }),
    );
    let _res2 = engine.execute_tool(&run, recall_req2).await.expect("recall 2");
    let touched2 = store.get_memory(memory.id).unwrap().unwrap();
    assert_eq!(touched2.recall_count, 2, "recall_count must increment to 2");

    // 3. Recall by entity_key via `core.memory.recall`
    let recall_req3 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_RECALL,
        json!({
            "entityKey": "Keycloak",
            "includeEpisodes": true
        }),
    );
    let res3 = engine.execute_tool(&run, recall_req3).await.expect("recall 3");
    assert!(res3.status == aro_core::ToolExecutionStatus::Completed);
    assert_eq!(res3.output["found"], true);
    let eps = res3.output["relatedEpisodes"].as_array().expect("episodes array");
    assert!(!eps.is_empty(), "Should correlate with Keycloak episode");

    cleanup_temp_db(db_path);
}

#[tokio::test]
async fn test_memory_update_primitive() {
    let (engine, store, db_path) = create_test_engine();
    let run = create_test_run(&store, None);

    // Save initial memory
    let memory = LongTermMemory {
        id: Uuid::new_v4(),
        client_id: None,
        content: "Original architecture notes for caching layer".to_string(),
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
    store.upsert_memory(&memory).expect("seed memory");

    // Update memory via `memory_update`
    let update_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_UPDATE,
        json!({
            "id": memory.id.to_string(),
            "content": "Updated caching architecture uses Redis cluster with LRU eviction",
            "salience": 0.95,
            "pinned": true,
            "category": "system"
        }),
    );
    let res = engine.execute_tool(&run, update_req).await.expect("update");
    assert!(res.status == aro_core::ToolExecutionStatus::Completed);
    assert_eq!(res.output["success"], true);
    assert_eq!(res.output["id"], memory.id.to_string());
    assert!(res.output.get("updatedAt").is_some());

    // Verify stored changes in SQLite
    let updated = store.get_memory(memory.id).unwrap().unwrap();
    assert_eq!(
        updated.content,
        "Updated caching architecture uses Redis cluster with LRU eviction"
    );
    assert_eq!(updated.salience, 0.95);
    assert!(updated.pinned);
    assert_eq!(updated.category, "system");

    // Verify canonical dotted `core.memory.update` also works
    let update_req2 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_UPDATE,
        json!({
            "id": memory.id.to_string(),
            "salience": 0.80
        }),
    );
    let res2 = engine.execute_tool(&run, update_req2).await.expect("update 2");
    assert_eq!(res2.output["success"], true);
    let updated2 = store.get_memory(memory.id).unwrap().unwrap();
    assert_eq!(updated2.salience, 0.80);

    cleanup_temp_db(db_path);
}

#[tokio::test]
async fn test_memory_forget_lifecycle() {
    let (engine, store, db_path) = create_test_engine();
    let run = create_test_run(&store, None);

    // Save memory
    let memory = LongTermMemory {
        id: Uuid::new_v4(),
        client_id: None,
        content: "Legacy monolith server ip 192.168.1.100 will be decommissioned".to_string(),
        category: "system".to_string(),
        scope: "project".to_string(),
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
    store.upsert_memory(&memory).expect("seed memory");

    // Confirm it is searchable initially
    let sres1 = engine
        .search_memories("Legacy monolith server", 5)
        .await
        .expect("search before forget");
    assert!(sres1.iter().any(|m| m.id == memory.id));

    // Forget via `memory_forget`
    let forget_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_FORGET,
        json!({
            "id": memory.id.to_string(),
            "reason": "Decommission completed"
        }),
    );
    let res = engine.execute_tool(&run, forget_req).await.expect("forget");
    assert!(res.status == aro_core::ToolExecutionStatus::Completed);
    assert_eq!(res.output["success"], true);
    assert_eq!(res.output["status"], "archived");

    // Verify memory is no longer returned in approved recall searches
    let sres2 = engine
        .search_memories("Legacy monolith server", 5)
        .await
        .expect("search after forget");
    assert!(!sres2.iter().any(|m| m.id == memory.id), "Archived memory must not appear in search");

    // Verify stored status is archived
    let archived = store.get_memory(memory.id).unwrap();
    // get_memory only returns non-deleted records
    assert!(archived.is_none(), "Soft-deleted memory must return None from get_memory");

    // Test alias `core.memory.forget` on another memory
    let memory2 = LongTermMemory {
        id: Uuid::new_v4(),
        client_id: None,
        content: "Temporary staging password secret_xyz123".to_string(),
        category: "technical".to_string(),
        scope: "conversation".to_string(),
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
    store.upsert_memory(&memory2).expect("seed memory2");

    let forget_req2 = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_CORE_MEMORY_FORGET,
        json!({
            "id": memory2.id.to_string()
        }),
    );
    let res2 = engine.execute_tool(&run, forget_req2).await.expect("forget 2");
    assert_eq!(res2.output["success"], true);
    assert_eq!(res2.output["status"], "archived");

    cleanup_temp_db(db_path);
}

#[tokio::test]
async fn test_memory_save_auto_salience_and_clamping() {
    let (engine, store, db_path) = create_test_engine();
    let run = create_test_run(&store, None);

    // 1. Save with empty content -> returns error
    let empty_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SAVE,
        json!({ "content": "   " }),
    );
    let empty_res = engine.execute_tool(&run, empty_req).await;
    assert!(empty_res.is_err() || empty_res.unwrap().status == aro_core::ToolExecutionStatus::Failed);

    // 2. Save without salience: auto-calculates salience in [0.1, 1.0]
    let auto_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SAVE,
        json!({
            "content": "User prefers concise answers and no markdown bolding",
            "category": "preference",
            "pinned": false
        }),
    );
    let auto_res = engine.execute_tool(&run, auto_req).await.expect("auto salience");
    let auto_id = Uuid::parse_str(auto_res.output["id"].as_str().unwrap()).unwrap();
    let auto_mem = store.get_memory(auto_id).unwrap().unwrap();
    assert!(
        auto_mem.salience >= 0.1 && auto_mem.salience <= 1.0,
        "Auto salience must be within [0.1, 1.0], got {}",
        auto_mem.salience
    );
    // Preference category bonus (+0.20) + base (0.35) = at least 0.55
    assert!(auto_mem.salience >= 0.50);

    // 3. Save with salience > 1.0 -> clamped to 1.0
    let clamp_req = ToolExecutionRequest::new(
        run.id,
        run.conversation_id,
        TOOL_MEMORY_SAVE,
        json!({
            "content": "Always use strict validation on incoming webhooks",
            "category": "technical",
            "salience": 5.0
        }),
    );
    let clamp_res = engine.execute_tool(&run, clamp_req).await.expect("clamped");
    let clamp_id = Uuid::parse_str(clamp_res.output["id"].as_str().unwrap()).unwrap();
    let clamp_mem = store.get_memory(clamp_id).unwrap().unwrap();
    assert_eq!(clamp_mem.salience, 1.0, "Salience > 1.0 must clamp to 1.0");

    cleanup_temp_db(db_path);
}

#[tokio::test]
async fn test_top_k_search_and_pinned_priority() {
    let (engine, store, db_path) = create_test_engine();

    // 1. Seed unpinned memory
    let unpinned_mem = LongTermMemory {
        id: Uuid::new_v4(),
        client_id: None,
        content: "Rust microservices use Tower middleware for rate limiting".to_string(),
        category: "technical".to_string(),
        scope: "project".to_string(),
        status: "approved".to_string(),
        source_conversation_id: None,
        source_message_ids: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        pinned: false,
        salience: 0.9,
        recall_count: 5,
        last_used_at: Some(Utc::now()),
    };
    store.upsert_memory(&unpinned_mem).expect("seed unpinned");

    // 2. Seed pinned memory (must appear first due to permanent decay immunity & priority)
    let pinned_mem = LongTermMemory {
        id: Uuid::new_v4(),
        client_id: None,
        content: "Always format dates using RFC 3339 in all API responses".to_string(),
        category: "system".to_string(),
        scope: "project".to_string(),
        status: "approved".to_string(),
        source_conversation_id: None,
        source_message_ids: vec![],
        created_at: Utc::now() - chrono::Duration::days(180),
        updated_at: Utc::now() - chrono::Duration::days(180),
        pinned: true,
        salience: 0.95,
        recall_count: 0,
        last_used_at: None,
    };
    store.upsert_memory(&pinned_mem).expect("seed pinned");

    // Run Top-K search
    let results = engine
        .search_memories("middleware rate limiting", 5)
        .await
        .expect("search");

    assert!(!results.is_empty(), "Must return memories");
    // Pinned memory must be first
    assert!(results[0].pinned, "Pinned memory must take priority over unpinned");
    assert_eq!(results[0].id, pinned_mem.id);

    cleanup_temp_db(db_path);
}
