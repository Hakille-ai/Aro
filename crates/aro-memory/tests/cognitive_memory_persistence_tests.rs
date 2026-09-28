use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use aro_core::{
    AgentArtifactRef, AgentMemoryContext, AgentMemoryFinding, AgentMessageEnvelope,
    AgentMessagePayload, AgentMessageType, AgentParticipant, AgentParticipantKind,
    AgentRunPriority, AssistantMode,
};
use aro_memory::SqliteMemoryStore;
use chrono::{Duration, Utc};
use uuid::Uuid;

struct TestDb {
    path: PathBuf,
}

impl Drop for TestDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn create_test_store() -> (SqliteMemoryStore, TestDb) {
    let path = std::env::temp_dir().join(format!("aro-cognitive-test-{}.sqlite", Uuid::new_v4()));
    let store = SqliteMemoryStore::new(&path).expect("create store");
    (store, TestDb { path })
}

#[test]
fn test_agent_memory_roundtrip() {
    let (store, _tmp) = create_test_store();
    let conv_id = "conv-101";
    let agent_id = "agent-arch";

    // 1. Initially non-existent
    let empty = store
        .get_agent_memory(conv_id, agent_id)
        .expect("get memory");
    assert!(empty.is_none());

    // 2. Create and populate full context
    let mut context = AgentMemoryContext::new(conv_id, agent_id, "System Architect", "architect");
    context.scratchpad = "Refactoring cognitive memory subsystem for persistence.".to_string();
    context.permission_profile_id = Some("profile-dev-autonomous".to_string());
    context.artifacts.push(AgentArtifactRef {
        id: "art-1".to_string(),
        title: "Memory Schema Blueprint".to_string(),
        kind: Some("design".to_string()),
        uri: Some("file:///docs/schema.md".to_string()),
    });

    let now = Utc::now();
    let finding1 = AgentMemoryFinding {
        id: "find-1".to_string(),
        summary: "SQLite WAL mode provides high concurrent read throughput".to_string(),
        category: Some("fact".to_string()),
        source_tool: Some("benchmark.run".to_string()),
        timestamp: now - Duration::seconds(10),
    };
    let finding2 = AgentMemoryFinding {
        id: "find-2".to_string(),
        summary: "Immediate transaction behavior prevents busy lockouts".to_string(),
        category: Some("decision".to_string()),
        source_tool: Some("audit.check".to_string()),
        timestamp: now,
    };
    context.findings = vec![finding1.clone(), finding2.clone()];

    let orchestrator = AgentParticipant::orchestrator("orch-root", "ARO Orchestrator");
    let subagent = AgentParticipant::subagent(
        agent_id,
        "System Architect",
        "architect",
        Some("🏗️".to_string()),
    );

    let envelope = AgentMessageEnvelope {
        id: "env-1".to_string(),
        conversation_id: conv_id.to_string(),
        parent_message_id: None,
        correlation_id: Some("corr-99".to_string()),
        sender: orchestrator.clone(),
        recipient: subagent.clone(),
        message_type: AgentMessageType::TaskDelegation,
        payload: AgentMessagePayload {
            content: "Implement table migrations and CRUD operations.".to_string(),
            structured_data: Some(serde_json::json!({ "priority": "high", "tier": 1 })),
            artifacts: vec![],
            suggested_actions: vec!["migrate".to_string(), "test".to_string()],
        },
        permission_profile_id: Some("profile-dev".to_string()),
        priority: Some(AgentRunPriority::High),
        timestamp: now - Duration::seconds(5),
    };
    context.ledger.push(envelope.clone());

    // 3. Save memory
    store.save_agent_memory(&context).expect("save memory");

    // 4. Retrieve and verify reconstitution
    let fetched = store
        .get_agent_memory(conv_id, agent_id)
        .expect("fetch memory")
        .expect("must exist");
    assert_eq!(fetched.agent_id, agent_id);
    assert_eq!(fetched.agent_name, "System Architect");
    assert_eq!(fetched.role, "architect");
    assert_eq!(fetched.conversation_id, conv_id);
    assert_eq!(
        fetched.scratchpad,
        "Refactoring cognitive memory subsystem for persistence."
    );
    assert_eq!(
        fetched.permission_profile_id,
        Some("profile-dev-autonomous".to_string())
    );
    assert_eq!(fetched.artifacts.len(), 1);
    assert_eq!(fetched.artifacts[0].title, "Memory Schema Blueprint");

    assert_eq!(fetched.findings.len(), 2);
    assert_eq!(fetched.findings[0].id, "find-1");
    assert_eq!(fetched.findings[0].category.as_deref(), Some("fact"));
    assert_eq!(fetched.findings[1].id, "find-2");
    assert_eq!(fetched.findings[1].category.as_deref(), Some("decision"));

    assert_eq!(fetched.ledger.len(), 1);
    assert_eq!(fetched.ledger[0].id, "env-1");
    assert_eq!(
        fetched.ledger[0].message_type,
        AgentMessageType::TaskDelegation
    );
    assert_eq!(
        fetched.ledger[0].payload.content,
        "Implement table migrations and CRUD operations."
    );
    assert_eq!(
        fetched.ledger[0].sender.kind,
        AgentParticipantKind::Orchestrator
    );
    assert_eq!(
        fetched.ledger[0].recipient.kind,
        AgentParticipantKind::Subagent
    );

    // 5. Inspect database tables counts
    let (mem_count, find_count, env_count) =
        store.inspect_cognitive_database_tables().expect("inspect");
    assert_eq!(mem_count, 1);
    assert_eq!(find_count, 2);
    assert_eq!(env_count, 1);
}

#[test]
fn test_scratchpad_partial_update() {
    let (store, _tmp) = create_test_store();
    let conv_id = "conv-102";
    let agent_id = "agent-qa";

    // Update scratchpad for non-existing context creates default record
    store
        .update_agent_scratchpad(conv_id, agent_id, "Executing test pass 1.")
        .expect("update scratchpad");

    let mem1 = store
        .get_agent_memory(conv_id, agent_id)
        .expect("get")
        .expect("exists");
    assert_eq!(mem1.scratchpad, "Executing test pass 1.");

    // Record a finding
    let finding = AgentMemoryFinding::new(
        "All 14 invariants passed.",
        Some("discovery".to_string()),
        None,
    );
    store
        .record_agent_finding(conv_id, agent_id, &finding)
        .expect("record finding");

    // Partial scratchpad update preserves existing findings
    store
        .update_agent_scratchpad(
            conv_id,
            agent_id,
            "Executing test pass 2 with adversarial fuzzing.",
        )
        .expect("update scratchpad second time");

    let mem2 = store
        .get_agent_memory(conv_id, agent_id)
        .expect("get")
        .expect("exists");
    assert_eq!(
        mem2.scratchpad,
        "Executing test pass 2 with adversarial fuzzing."
    );
    assert_eq!(mem2.findings.len(), 1);
    assert_eq!(mem2.findings[0].summary, "All 14 invariants passed.");
}

#[test]
fn test_finding_deduplication_and_upsert() {
    let (store, _tmp) = create_test_store();
    let conv_id = "conv-103";
    let agent_id = "agent-sec";

    let mut finding = AgentMemoryFinding::new(
        "Vulnerability identified in input parser",
        Some("vulnerability".to_string()),
        Some("scanner".to_string()),
    );
    let finding_id = finding.id.clone();
    store
        .record_agent_finding(conv_id, agent_id, &finding)
        .expect("record");

    // Update same finding ID
    finding.summary = "Vulnerability resolved with strict path confinement".to_string();
    finding.category = Some("fact".to_string());
    store
        .record_agent_finding(conv_id, agent_id, &finding)
        .expect("upsert finding");

    let findings = store
        .list_agent_findings(conv_id, Some(agent_id))
        .expect("list findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].id, finding_id);
    assert_eq!(
        findings[0].summary,
        "Vulnerability resolved with strict path confinement"
    );
    assert_eq!(findings[0].category.as_deref(), Some("fact"));
}

#[test]
fn test_ledger_filtering_and_broadcast() {
    let (store, _tmp) = create_test_store();
    let conv_id = "conv-104";

    let orch = AgentParticipant::orchestrator("orch", "Orchestrator");
    let sub1 = AgentParticipant::subagent("sub1", "Worker 1", "worker", None);
    let sub2 = AgentParticipant::subagent("sub2", "Worker 2", "worker", None);
    let broadcast = AgentParticipant {
        id: "broadcast".to_string(),
        name: "Broadcast".to_string(),
        role: None,
        icon: None,
        kind: AgentParticipantKind::Broadcast,
    };

    let env1 = AgentMessageEnvelope::new(
        conv_id,
        orch.clone(),
        sub1.clone(),
        AgentMessageType::TaskDelegation,
        "Task 1",
    );
    let env2 = AgentMessageEnvelope::new(
        conv_id,
        sub1.clone(),
        orch.clone(),
        AgentMessageType::TaskProgress,
        "Progress 50%",
    );
    let env3 = AgentMessageEnvelope::new(
        conv_id,
        sub1.clone(),
        sub2.clone(),
        AgentMessageType::PeerCollaboration,
        "Data handover",
    );
    let env_bcast = AgentMessageEnvelope::new(
        conv_id,
        orch.clone(),
        broadcast.clone(),
        AgentMessageType::ContextShare,
        "Global announcement",
    );

    store.record_agent_envelope(&env1).expect("rec 1");
    store.record_agent_envelope(&env2).expect("rec 2");
    store.record_agent_envelope(&env3).expect("rec 3");
    store.record_agent_envelope(&env_bcast).expect("rec bcast");

    // Sub1 ledger should contain env1 (recipient), env2 (sender), env3 (sender), and env_bcast (broadcast)
    let sub1_ledger = store
        .list_agent_envelopes(conv_id, Some("sub1"), None)
        .expect("list sub1");
    assert_eq!(sub1_ledger.len(), 4);

    // Sub2 ledger should contain env3 (recipient) and env_bcast (broadcast)
    let sub2_ledger = store
        .list_agent_envelopes(conv_id, Some("sub2"), None)
        .expect("list sub2");
    assert_eq!(sub2_ledger.len(), 2);

    // Filter by message type
    let delegation_only = store
        .list_agent_envelopes(conv_id, None, Some(&AgentMessageType::TaskDelegation))
        .expect("list delegations");
    assert_eq!(delegation_only.len(), 1);
    assert_eq!(delegation_only[0].id, env1.id);
}

#[test]
fn test_clear_agent_memory_and_delete_conversation_cascade() {
    let (store, _tmp) = create_test_store();

    let conv = store
        .create_conversation("Multi-agent session", AssistantMode::Chat, None, None)
        .expect("create conv");
    let conv_id = conv.id.to_string();

    let sub1 = AgentMemoryContext::new(&conv_id, "worker-1", "W1", "role");
    let sub2 = AgentMemoryContext::new(&conv_id, "worker-2", "W2", "role");
    store.save_agent_memory(&sub1).expect("save 1");
    store.save_agent_memory(&sub2).expect("save 2");

    let (mem_count, _, _) = store.inspect_cognitive_database_tables().expect("inspect");
    assert_eq!(mem_count, 2);

    // 1. Clear memory for single agent
    store
        .clear_agent_memory(&conv_id, Some("worker-1"))
        .expect("clear worker 1");
    assert!(store
        .get_agent_memory(&conv_id, "worker-1")
        .expect("get")
        .is_none());
    assert!(store
        .get_agent_memory(&conv_id, "worker-2")
        .expect("get")
        .is_some());

    // 2. Cascade delete on conversation removal
    store
        .delete_conversation(conv.id)
        .expect("delete conversation");
    let (mem_count_after, find_count_after, env_count_after) = store
        .inspect_cognitive_database_tables()
        .expect("inspect after delete");
    assert_eq!(mem_count_after, 0);
    assert_eq!(find_count_after, 0);
    assert_eq!(env_count_after, 0);
}

#[test]
fn test_concurrent_writes_immediate_isolation() {
    let (store, _tmp) = create_test_store();
    let store = Arc::new(store);
    let conv_id = "conv-stress";

    let mut handles = Vec::new();
    for thread_idx in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = thread::spawn(move || {
            let agent_id = format!("agent-{}", thread_idx);
            for step in 0..20 {
                let scratch = format!("Step {} computation result", step);
                store_clone
                    .update_agent_scratchpad(conv_id, &agent_id, &scratch)
                    .expect("concurrent scratchpad update");

                let finding = AgentMemoryFinding::new(
                    format!("Agent {} discovery at iteration {}", thread_idx, step),
                    Some("fact".to_string()),
                    Some("tool".to_string()),
                );
                store_clone
                    .record_agent_finding(conv_id, &agent_id, &finding)
                    .expect("concurrent finding recording");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("thread join");
    }

    let (mem_count, find_count, _) = store.inspect_cognitive_database_tables().expect("inspect");
    assert_eq!(mem_count, 10);
    assert_eq!(find_count, 200);
}

#[test]
fn test_reset_wipes_all_cognitive_memory() {
    let (store, _tmp) = create_test_store();
    let conv_id = "conv-reset";
    let mem = AgentMemoryContext::new(conv_id, "agent-1", "Worker", "worker");
    store.save_agent_memory(&mem).expect("save");

    let finding = AgentMemoryFinding::new("Observation before reset", None, None);
    store
        .record_agent_finding(conv_id, "agent-1", &finding)
        .expect("record finding");

    let (m, f, _) = store
        .inspect_cognitive_database_tables()
        .expect("count before");
    assert!(m > 0 && f > 0);

    store.reset().expect("reset");

    let (m_after, f_after, e_after) = store
        .inspect_cognitive_database_tables()
        .expect("count after");
    assert_eq!(m_after, 0);
    assert_eq!(f_after, 0);
    assert_eq!(e_after, 0);
}
