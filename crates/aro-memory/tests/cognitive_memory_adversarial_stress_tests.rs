use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

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
        let _ = std::fs::remove_file(format!("{}-wal", self.path.to_string_lossy()));
        let _ = std::fs::remove_file(format!("{}-shm", self.path.to_string_lossy()));
    }
}

fn create_test_store() -> (SqliteMemoryStore, TestDb) {
    let path = std::env::temp_dir().join(format!(
        "aro-cognitive-adversarial-test-{}.sqlite",
        Uuid::new_v4()
    ));
    let store = SqliteMemoryStore::new(&path).expect("create store");
    (store, TestDb { path })
}

#[test]
fn test_extreme_inputs_empty_and_massive_payloads() {
    let (store, _tmp) = create_test_store();
    let conv_id = "conv-extreme-001";
    let agent_id = "agent-adversary";

    // 1. Empty strings roundtrip
    let mut empty_ctx = AgentMemoryContext::new(conv_id, agent_id, "", "");
    empty_ctx.scratchpad = String::new();
    empty_ctx.findings.push(AgentMemoryFinding {
        id: "find-empty".to_string(),
        summary: String::new(),
        category: None,
        source_tool: None,
        timestamp: Utc::now(),
    });
    empty_ctx.ledger.push(AgentMessageEnvelope {
        id: "env-empty".to_string(),
        conversation_id: conv_id.to_string(),
        parent_message_id: None,
        correlation_id: None,
        sender: AgentParticipant::subagent(agent_id, "", "", None),
        recipient: AgentParticipant::orchestrator("orch", ""),
        message_type: AgentMessageType::TaskDelegation,
        payload: AgentMessagePayload {
            content: String::new(),
            structured_data: None,
            artifacts: vec![],
            suggested_actions: vec![],
        },
        permission_profile_id: None,
        priority: None,
        timestamp: Utc::now(),
    });

    store.save_agent_memory(&empty_ctx).expect("save empty ctx");
    let fetched_empty = store
        .get_agent_memory(conv_id, agent_id)
        .expect("get empty ctx")
        .expect("must exist");
    assert_eq!(fetched_empty.scratchpad, "");
    assert_eq!(fetched_empty.agent_name, "");
    assert_eq!(fetched_empty.role, "");
    assert_eq!(fetched_empty.findings.len(), 1);
    assert_eq!(fetched_empty.findings[0].summary, "");
    assert_eq!(fetched_empty.findings[0].category, None);
    assert_eq!(fetched_empty.ledger.len(), 1);
    assert_eq!(fetched_empty.ledger[0].payload.content, "");

    // 2. Massive payloads (>120,000 chars) with special characters and Unicode
    let conv_massive_id = "conv-extreme-massive-002";
    let agent_massive_id = "agent-massive";

    let massive_scratchpad = format!(
        "🧠 Multi-byte Astral Unicode & Special SQL Injections: '; DROP TABLE agent_memories; -- \
        \"quotes\" 'single' `backticks` \\ backslashes / slashes \r\n newlines \t tabs \
        CJK: 日本語のテストと中国語のテスト; Arabic: مرحبا بالعالم; Cyrillic: русский текст; \
        Math/Greek: ∑∏∫∂∇ αβγδε; Payload: {}",
        "X".repeat(120_000)
    );
    assert!(massive_scratchpad.chars().count() > 120_000);

    let massive_finding_summary = format!(
        "⚡ Massive Finding Summary with nested JSON and HTML entities: <script>alert('xss')</script> \
        {{\"nested\": {{\"key\": \"value\", \"array\": [1, 2, 3]}}}} \
        Payload: {}",
        "Y".repeat(100_000)
    );
    assert!(massive_finding_summary.chars().count() > 100_000);

    let massive_envelope_content = format!(
        "🚀 Massive Envelope Content: '; DELETE FROM agent_message_envelopes; -- \
        Payload: {}",
        "Z".repeat(100_000)
    );
    assert!(massive_envelope_content.chars().count() > 100_000);

    let mut massive_ctx = AgentMemoryContext::new(
        conv_massive_id,
        agent_massive_id,
        "Super Agent 🔥",
        "Lead Adversarial Tester 🛡️",
    );
    massive_ctx.scratchpad = massive_scratchpad.clone();
    massive_ctx.permission_profile_id =
        Some("profile-dev-'; DROP TABLE permission_profiles; --".to_string());
    massive_ctx.artifacts.push(AgentArtifactRef {
        id: "art-massive-1".to_string(),
        title: "Massive Artifact Blueprint with Unicode 🎨".to_string(),
        kind: Some("code/rust".to_string()),
        uri: Some("file:///C:/path/with spaces/and'quotes/file.rs".to_string()),
    });

    let finding_id = "find-massive-001".to_string();
    let finding_cat = Some("discovery-🔥-special".to_string());
    let finding_tool = Some("tool.custom_exec('arg1', 'arg2')".to_string());
    massive_ctx.findings.push(AgentMemoryFinding {
        id: finding_id.clone(),
        summary: massive_finding_summary.clone(),
        category: finding_cat.clone(),
        source_tool: finding_tool.clone(),
        timestamp: Utc::now(),
    });

    let env_id = "env-massive-001".to_string();
    massive_ctx.ledger.push(AgentMessageEnvelope {
        id: env_id.clone(),
        conversation_id: conv_massive_id.to_string(),
        parent_message_id: Some("parent-env-000".to_string()),
        correlation_id: Some("corr-id-xyz-123".to_string()),
        sender: AgentParticipant::subagent(
            agent_massive_id,
            "Super Agent 🔥",
            "tester",
            Some("🛡️".to_string()),
        ),
        recipient: AgentParticipant::orchestrator("orch-root", "ARO Orchestrator 👑"),
        message_type: AgentMessageType::TaskProgress,
        payload: AgentMessagePayload {
            content: massive_envelope_content.clone(),
            structured_data: Some(serde_json::json!({
                "raw_sql_test": "SELECT * FROM users WHERE '1'='1';",
                "massive_nested": {
                    "inner": "A".repeat(10_000)
                }
            })),
            artifacts: vec![],
            suggested_actions: vec![
                "run_tests".to_string(),
                "validate_integrity(); --".to_string(),
            ],
        },
        permission_profile_id: Some("profile-unrestricted".to_string()),
        priority: Some(AgentRunPriority::High),
        timestamp: Utc::now(),
    });

    // Save massive context
    let start_save = Instant::now();
    store
        .save_agent_memory(&massive_ctx)
        .expect("save massive context");
    let save_duration = start_save.elapsed();
    println!("Massive context save took: {:?}", save_duration);

    // Retrieve and verify bit-for-bit exact reconstitution
    let start_fetch = Instant::now();
    let fetched = store
        .get_agent_memory(conv_massive_id, agent_massive_id)
        .expect("fetch massive context")
        .expect("massive context must exist");
    let fetch_duration = start_fetch.elapsed();
    println!("Massive context fetch took: {:?}", fetch_duration);

    assert_eq!(fetched.scratchpad, massive_scratchpad);
    assert_eq!(fetched.agent_name, "Super Agent 🔥");
    assert_eq!(fetched.role, "Lead Adversarial Tester 🛡️");
    assert_eq!(
        fetched.permission_profile_id,
        Some("profile-dev-'; DROP TABLE permission_profiles; --".to_string())
    );
    assert_eq!(fetched.artifacts.len(), 1);
    assert_eq!(
        fetched.artifacts[0].title,
        "Massive Artifact Blueprint with Unicode 🎨"
    );

    assert_eq!(fetched.findings.len(), 1);
    assert_eq!(fetched.findings[0].id, finding_id);
    assert_eq!(fetched.findings[0].summary, massive_finding_summary);
    assert_eq!(fetched.findings[0].category, finding_cat);
    assert_eq!(fetched.findings[0].source_tool, finding_tool);

    assert_eq!(fetched.ledger.len(), 1);
    assert_eq!(fetched.ledger[0].id, env_id);
    assert_eq!(fetched.ledger[0].payload.content, massive_envelope_content);
    assert_eq!(fetched.ledger[0].sender.name, "Super Agent 🔥");
    assert_eq!(fetched.ledger[0].recipient.name, "ARO Orchestrator 👑");

    // Verify raw table counts (empty + massive)
    let (mem_count, find_count, env_count) =
        store.inspect_cognitive_database_tables().expect("inspect");
    assert_eq!(mem_count, 2);
    assert_eq!(find_count, 2);
    assert_eq!(env_count, 2);
}

#[test]
fn test_concurrent_writes_immediate_isolation_heavy_stress() {
    let (store, _tmp) = create_test_store();
    let store = Arc::new(store);
    let conv_id = "conv-concurrent-stress";

    let num_workers = 12;
    let iterations_per_worker = 50;
    let total_expected_ops = num_workers * iterations_per_worker;

    let write_errors = Arc::new(AtomicUsize::new(0));
    let read_errors = Arc::new(AtomicUsize::new(0));
    let successful_ops = Arc::new(AtomicUsize::new(0));

    let start_time = Instant::now();
    let mut handles = Vec::new();

    for worker_idx in 0..num_workers {
        let store_clone = Arc::clone(&store);
        let write_errors_clone = Arc::clone(&write_errors);
        let read_errors_clone = Arc::clone(&read_errors);
        let successful_ops_clone = Arc::clone(&successful_ops);

        let handle = thread::spawn(move || {
            let agent_id = format!("agent-worker-{:02}", worker_idx);

            for iter in 0..iterations_per_worker {
                match worker_idx % 4 {
                    0 => {
                        // Scratchpad update writer
                        let scratch = format!(
                            "Worker {} iter {} scratchpad computation result at {:?}",
                            worker_idx,
                            iter,
                            Utc::now()
                        );
                        if let Err(e) =
                            store_clone.update_agent_scratchpad(conv_id, &agent_id, &scratch)
                        {
                            eprintln!("Scratchpad write error: {:?}", e);
                            write_errors_clone.fetch_add(1, Ordering::Relaxed);
                        } else {
                            successful_ops_clone.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    1 => {
                        // Finding recorder writer
                        let finding = AgentMemoryFinding::new(
                            format!(
                                "Finding from worker {} iter {}: discovered optimal index",
                                worker_idx, iter
                            ),
                            Some(if iter % 2 == 0 {
                                "discovery".to_string()
                            } else {
                                "fact".to_string()
                            }),
                            Some("profiler.cpu".to_string()),
                        );
                        if let Err(e) =
                            store_clone.record_agent_finding(conv_id, &agent_id, &finding)
                        {
                            eprintln!("Finding write error: {:?}", e);
                            write_errors_clone.fetch_add(1, Ordering::Relaxed);
                        } else {
                            successful_ops_clone.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    2 => {
                        // Envelope recorder writer
                        let recipient_id =
                            format!("agent-worker-{:02}", (worker_idx + 1) % num_workers);
                        let env = AgentMessageEnvelope::new(
                            conv_id,
                            AgentParticipant::subagent(
                                &agent_id,
                                format!("Worker {}", worker_idx),
                                "worker",
                                None,
                            ),
                            AgentParticipant::subagent(
                                &recipient_id,
                                "Next Worker",
                                "worker",
                                None,
                            ),
                            AgentMessageType::PeerCollaboration,
                            format!("Sync token {} from worker {}", iter, worker_idx),
                        );
                        if let Err(e) = store_clone.record_agent_envelope(&env) {
                            eprintln!("Envelope write error: {:?}", e);
                            write_errors_clone.fetch_add(1, Ordering::Relaxed);
                        } else {
                            successful_ops_clone.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    _ => {
                        // Full context saver & interleaved reader
                        let mut ctx = AgentMemoryContext::new(
                            conv_id,
                            &agent_id,
                            format!("Worker {}", worker_idx),
                            "collaborator",
                        );
                        ctx.scratchpad = format!("Full context sync iter {}", iter);
                        if let Err(e) = store_clone.save_agent_memory(&ctx) {
                            eprintln!("Full context save error: {:?}", e);
                            write_errors_clone.fetch_add(1, Ordering::Relaxed);
                        } else {
                            successful_ops_clone.fetch_add(1, Ordering::Relaxed);
                        }

                        // Interleaved concurrent read
                        if let Err(e) = store_clone.get_agent_memory(conv_id, &agent_id) {
                            eprintln!("Concurrent read error: {:?}", e);
                            read_errors_clone.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("thread must join cleanly");
    }

    let elapsed = start_time.elapsed();
    let total_writes_ok = successful_ops.load(Ordering::SeqCst);
    let total_w_errs = write_errors.load(Ordering::SeqCst);
    let total_r_errs = read_errors.load(Ordering::SeqCst);

    println!(
        "Concurrent Cognitive Persistence Stress: {} successful ops (write_errs={}, read_errs={}) in {:?}",
        total_writes_ok, total_w_errs, total_r_errs, elapsed
    );

    assert_eq!(
        total_w_errs, 0,
        "TransactionBehavior::Immediate must produce zero SQLITE_BUSY / lock write errors"
    );
    assert_eq!(
        total_r_errs, 0,
        "WAL mode must allow simultaneous readers with zero lock read errors"
    );
    assert_eq!(total_writes_ok, total_expected_ops);

    let (mem_count, find_count, env_count) = store
        .inspect_cognitive_database_tables()
        .expect("inspect final state");
    println!(
        "Final table state after concurrent stress: memories={}, findings={}, envelopes={}",
        mem_count, find_count, env_count
    );
    assert!(mem_count > 0);
    assert_eq!(find_count, (num_workers / 4) * iterations_per_worker);
    assert_eq!(env_count, (num_workers / 4) * iterations_per_worker);
}

#[test]
fn test_cascade_deletions_multi_conversation_isolation() {
    let (store, _tmp) = create_test_store();

    // 1. Create two formal conversations in SQLite
    let conv1 = store
        .create_conversation("Conversation Alpha", AssistantMode::Chat, None, None)
        .expect("create conv1");
    let conv2 = store
        .create_conversation("Conversation Beta", AssistantMode::Chat, None, None)
        .expect("create conv2");

    let conv1_id = conv1.id.to_string();
    let conv2_id = conv2.id.to_string();

    // 2. Populate Conversation Alpha (3 agents, 6 findings, 4 envelopes)
    for i in 1..=3 {
        let aid = format!("agent-alpha-{}", i);
        let mut ctx =
            AgentMemoryContext::new(&conv1_id, &aid, format!("Alpha Agent {}", i), "worker");
        ctx.scratchpad = format!("Alpha scratchpad {}", i);
        store.save_agent_memory(&ctx).expect("save alpha mem");

        // 2 findings per agent
        let f1 = AgentMemoryFinding::new(format!("Alpha {} fact 1", i), Some("fact".into()), None);
        let f2 = AgentMemoryFinding::new(format!("Alpha {} fact 2", i), Some("fact".into()), None);
        store
            .record_agent_finding(&conv1_id, &aid, &f1)
            .expect("f1");
        store
            .record_agent_finding(&conv1_id, &aid, &f2)
            .expect("f2");
    }

    // 4 envelopes in Alpha
    for i in 1..=4 {
        let env = AgentMessageEnvelope::new(
            &conv1_id,
            AgentParticipant::subagent("agent-alpha-1", "Alpha 1", "worker", None),
            AgentParticipant::subagent("agent-alpha-2", "Alpha 2", "worker", None),
            AgentMessageType::TaskDelegation,
            format!("Alpha delegation {}", i),
        );
        store.record_agent_envelope(&env).expect("alpha env");
    }

    // 3. Populate Conversation Beta (2 agents, 4 findings, 3 envelopes)
    for i in 1..=2 {
        let aid = format!("agent-beta-{}", i);
        let mut ctx =
            AgentMemoryContext::new(&conv2_id, &aid, format!("Beta Agent {}", i), "worker");
        ctx.scratchpad = format!("Beta scratchpad {}", i);
        store.save_agent_memory(&ctx).expect("save beta mem");

        let f1 = AgentMemoryFinding::new(format!("Beta {} fact 1", i), Some("fact".into()), None);
        let f2 = AgentMemoryFinding::new(format!("Beta {} fact 2", i), Some("fact".into()), None);
        store
            .record_agent_finding(&conv2_id, &aid, &f1)
            .expect("f1 beta");
        store
            .record_agent_finding(&conv2_id, &aid, &f2)
            .expect("f2 beta");
    }

    // 3 envelopes in Beta
    for i in 1..=3 {
        let env = AgentMessageEnvelope::new(
            &conv2_id,
            AgentParticipant::subagent("agent-beta-1", "Beta 1", "worker", None),
            AgentParticipant::subagent("agent-beta-2", "Beta 2", "worker", None),
            AgentMessageType::TaskDelegation,
            format!("Beta delegation {}", i),
        );
        store.record_agent_envelope(&env).expect("beta env");
    }

    // Verify initial aggregated table counts
    let (m_init, f_init, e_init) = store
        .inspect_cognitive_database_tables()
        .expect("init counts");
    assert_eq!(m_init, 5); // 3 alpha + 2 beta
    assert_eq!(f_init, 10); // 6 alpha + 4 beta
    assert_eq!(e_init, 7); // 4 alpha + 3 beta

    // 4. Test Single Agent Clear inside Alpha
    // Clear only agent-alpha-1
    store
        .clear_agent_memory(&conv1_id, Some("agent-alpha-1"))
        .expect("clear agent-alpha-1");

    // agent-alpha-1 memory should be gone
    assert!(store
        .get_agent_memory(&conv1_id, "agent-alpha-1")
        .unwrap()
        .is_none());
    // agent-alpha-2 and agent-alpha-3 memory must remain intact
    assert!(store
        .get_agent_memory(&conv1_id, "agent-alpha-2")
        .unwrap()
        .is_some());
    assert!(store
        .get_agent_memory(&conv1_id, "agent-alpha-3")
        .unwrap()
        .is_some());
    // Beta agents must remain completely untouched
    assert!(store
        .get_agent_memory(&conv2_id, "agent-beta-1")
        .unwrap()
        .is_some());
    assert!(store
        .get_agent_memory(&conv2_id, "agent-beta-2")
        .unwrap()
        .is_some());

    // 5. Test Full Conversation Cascade Deletion for Alpha
    // Deleting conv1 must purge all remaining alpha memories, findings, and envelopes
    store.delete_conversation(conv1.id).expect("delete conv1");

    // All Alpha records must be gone
    assert!(store
        .get_agent_memory(&conv1_id, "agent-alpha-2")
        .unwrap()
        .is_none());
    assert!(store
        .get_agent_memory(&conv1_id, "agent-alpha-3")
        .unwrap()
        .is_none());
    assert_eq!(store.list_agent_findings(&conv1_id, None).unwrap().len(), 0);
    assert_eq!(
        store
            .list_agent_envelopes(&conv1_id, None, None)
            .unwrap()
            .len(),
        0
    );

    // All Beta records must be 100% intact!
    let (m_after_alpha, f_after_alpha, e_after_alpha) = store
        .inspect_cognitive_database_tables()
        .expect("counts after alpha delete");
    assert_eq!(
        m_after_alpha, 2,
        "Only Beta's 2 agent memories should remain"
    );
    assert_eq!(f_after_alpha, 4, "Only Beta's 4 findings should remain");
    assert_eq!(e_after_alpha, 3, "Only Beta's 3 envelopes should remain");

    let beta_mem1 = store
        .get_agent_memory(&conv2_id, "agent-beta-1")
        .unwrap()
        .unwrap();
    assert_eq!(beta_mem1.scratchpad, "Beta scratchpad 1");

    // 6. Test Cascade Deletion for Beta
    store.delete_conversation(conv2.id).expect("delete conv2");

    let (m_final, f_final, e_final) = store
        .inspect_cognitive_database_tables()
        .expect("final counts");
    assert_eq!(
        m_final, 0,
        "All agent memories must be 0 after deleting all conversations"
    );
    assert_eq!(f_final, 0, "All agent findings must be 0");
    assert_eq!(e_final, 0, "All agent envelopes must be 0");
}

#[test]
fn test_finding_idempotency_and_extreme_upsert() {
    let (store, _tmp) = create_test_store();
    let conv_id = "conv-finding-upsert";
    let agent_id = "agent-finding-tester";

    let finding_id = "finding-immutable-key-001";

    // 1. Initial short finding
    let mut finding = AgentMemoryFinding {
        id: finding_id.to_string(),
        summary: "Short summary".to_string(),
        category: Some("preliminary".to_string()),
        source_tool: Some("tool_a".to_string()),
        timestamp: Utc::now() - Duration::hours(1),
    };
    store
        .record_agent_finding(conv_id, agent_id, &finding)
        .expect("record short finding");

    let list1 = store
        .list_agent_findings(conv_id, Some(agent_id))
        .expect("list 1");
    assert_eq!(list1.len(), 1);
    assert_eq!(list1[0].summary, "Short summary");

    // 2. Extreme upsert: update same finding with 100,000+ chars
    finding.summary = "M".repeat(110_000);
    finding.category = Some("exhaustive_empirical_stress_result".to_string());
    finding.source_tool = Some("tool_b_extreme".to_string());
    finding.timestamp = Utc::now();

    store
        .record_agent_finding(conv_id, agent_id, &finding)
        .expect("upsert massive finding");

    let list2 = store
        .list_agent_findings(conv_id, Some(agent_id))
        .expect("list 2");
    assert_eq!(
        list2.len(),
        1,
        "Deduplication must keep exactly 1 row on ID conflict"
    );
    assert_eq!(list2[0].id, finding_id);
    assert_eq!(list2[0].summary.len(), 110_000);
    assert_eq!(
        list2[0].category.as_deref(),
        Some("exhaustive_empirical_stress_result")
    );
    assert_eq!(list2[0].source_tool.as_deref(), Some("tool_b_extreme"));

    // 3. Upsert with empty ID auto-generates distinct UUIDs
    let auto1 = AgentMemoryFinding::new("Auto ID finding 1", None, None);
    let auto2 = AgentMemoryFinding::new("Auto ID finding 2", None, None);
    assert_ne!(auto1.id, auto2.id);

    store
        .record_agent_finding(conv_id, agent_id, &auto1)
        .expect("record auto 1");
    store
        .record_agent_finding(conv_id, agent_id, &auto2)
        .expect("record auto 2");

    let list3 = store
        .list_agent_findings(conv_id, Some(agent_id))
        .expect("list 3");
    assert_eq!(list3.len(), 3);
}

#[test]
fn test_envelope_ledger_isolation_and_broadcast_routing() {
    let (store, _tmp) = create_test_store();
    let conv_id = "conv-envelope-routing";

    let orch = AgentParticipant::orchestrator("orch", "Orchestrator");
    let agent_a = AgentParticipant::subagent("agent-a", "Agent Alpha", "worker", None);
    let agent_b = AgentParticipant::subagent("agent-b", "Agent Beta", "worker", None);
    let agent_c = AgentParticipant::subagent("agent-c", "Agent Gamma", "worker", None);
    let broadcast = AgentParticipant {
        id: "broadcast-hub".to_string(),
        name: "Broadcast Channel".to_string(),
        role: None,
        icon: None,
        kind: AgentParticipantKind::Broadcast,
    };

    // Envelope 1: Orch -> A (Delegation)
    let env_orch_a = AgentMessageEnvelope::new(
        conv_id,
        orch.clone(),
        agent_a.clone(),
        AgentMessageType::TaskDelegation,
        "Analyze memory logs",
    );
    // Envelope 2: A -> B (Peer Collaboration)
    let env_a_b = AgentMessageEnvelope::new(
        conv_id,
        agent_a.clone(),
        agent_b.clone(),
        AgentMessageType::PeerCollaboration,
        "Here are intermediate observations",
    );
    // Envelope 3: B -> Orch (Task Progress)
    let env_b_orch = AgentMessageEnvelope::new(
        conv_id,
        agent_b.clone(),
        orch.clone(),
        AgentMessageType::TaskProgress,
        "50% done",
    );
    // Envelope 4: Orch -> Broadcast (Context Share)
    let env_bcast = AgentMessageEnvelope::new(
        conv_id,
        orch.clone(),
        broadcast.clone(),
        AgentMessageType::ContextShare,
        "Global architecture milestone 1 unlocked",
    );

    store.record_agent_envelope(&env_orch_a).expect("rec 1");
    store.record_agent_envelope(&env_a_b).expect("rec 2");
    store.record_agent_envelope(&env_b_orch).expect("rec 3");
    store.record_agent_envelope(&env_bcast).expect("rec 4");

    // Agent A ledger:
    // - env_orch_a (recipient)
    // - env_a_b (sender)
    // - env_bcast (broadcast)
    // MUST NOT contain env_b_orch!
    let ledger_a = store
        .list_agent_envelopes(conv_id, Some("agent-a"), None)
        .expect("ledger a");
    assert_eq!(ledger_a.len(), 3);
    let ids_a: Vec<String> = ledger_a.into_iter().map(|e| e.id).collect();
    assert!(ids_a.contains(&env_orch_a.id));
    assert!(ids_a.contains(&env_a_b.id));
    assert!(ids_a.contains(&env_bcast.id));
    assert!(!ids_a.contains(&env_b_orch.id));

    // Agent C ledger:
    // Only broadcast! Private messages between Orch, A, and B must NEVER leak to C
    let ledger_c = store
        .list_agent_envelopes(conv_id, Some(&agent_c.id), None)
        .expect("ledger c");
    assert_eq!(
        ledger_c.len(),
        1,
        "Agent C must only receive broadcast message"
    );
    assert_eq!(ledger_c[0].id, env_bcast.id);

    // Global conversation ledger (agent_id = None)
    let ledger_all = store
        .list_agent_envelopes(conv_id, None, None)
        .expect("ledger all");
    assert_eq!(ledger_all.len(), 4);

    // Filter by type: TaskDelegation
    let delegations = store
        .list_agent_envelopes(conv_id, None, Some(&AgentMessageType::TaskDelegation))
        .expect("delegations");
    assert_eq!(delegations.len(), 1);
    assert_eq!(delegations[0].id, env_orch_a.id);
}

#[test]
fn test_clear_memory_nonexistent_and_edge_conditions() {
    let (store, _tmp) = create_test_store();

    // 1. Clearing non-existent conversation or agent must succeed cleanly (no error)
    let res1 = store.clear_agent_memory("non-existent-conv", Some("ghost-agent"));
    assert!(res1.is_ok(), "Clearing non-existent agent must not fail");

    let res2 = store.clear_agent_memory("non-existent-conv", None);
    assert!(
        res2.is_ok(),
        "Clearing non-existent conversation must not fail"
    );

    // 2. get_agent_memory on non-existent returns Ok(None)
    let res3 = store
        .get_agent_memory("non-existent-conv", "ghost-agent")
        .unwrap();
    assert!(res3.is_none());

    // 3. get_or_create creates default with clean defaults
    let created = store
        .get_or_create_agent_memory(
            "conv-default",
            "agent-default",
            "Default Name",
            "default_role",
        )
        .expect("get or create");
    assert_eq!(created.agent_id, "agent-default");
    assert_eq!(created.agent_name, "Default Name");
    assert_eq!(created.role, "default_role");
    assert_eq!(created.scratchpad, "");
    assert!(created.findings.is_empty());
    assert!(created.ledger.is_empty());

    // 4. Repeated get_or_create returns identical existing record without creating duplicates
    let fetched = store
        .get_or_create_agent_memory(
            "conv-default",
            "agent-default",
            "Different Name",
            "different_role",
        )
        .expect("get or create existing");
    assert_eq!(
        fetched.agent_name, "Default Name",
        "Should retain original name"
    );
    assert_eq!(fetched.role, "default_role");

    let (m, _, _) = store.inspect_cognitive_database_tables().expect("inspect");
    assert_eq!(m, 1);
}
