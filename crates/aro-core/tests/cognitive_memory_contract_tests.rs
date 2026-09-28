use aro_core::{
    AgentArtifactRef, AgentMemoryContext, AgentMemoryFinding, AgentMessageEnvelope,
    AgentMessagePayload, AgentMessageType, AgentParticipant, AgentParticipantKind,
    AgentRunPriority,
};
use chrono::{TimeZone, Utc};
use serde_json::json;

#[test]
fn test_agent_message_type_serialization() {
    let types = vec![
        (AgentMessageType::TaskDelegation, "\"task_delegation\""),
        (AgentMessageType::TaskProgress, "\"task_progress\""),
        (AgentMessageType::TaskResult, "\"task_result\""),
        (
            AgentMessageType::ClarificationRequest,
            "\"clarification_request\"",
        ),
        (
            AgentMessageType::ClarificationResponse,
            "\"clarification_response\"",
        ),
        (
            AgentMessageType::PeerCollaboration,
            "\"peer_collaboration\"",
        ),
        (AgentMessageType::ContextQuery, "\"context_query\""),
        (AgentMessageType::ContextShare, "\"context_share\""),
        (AgentMessageType::ErrorEscalation, "\"error_escalation\""),
    ];

    for (variant, expected_json) in types {
        let serialized = serde_json::to_string(&variant).unwrap();
        assert_eq!(serialized, expected_json);
        let deserialized: AgentMessageType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, variant);
    }
}

#[test]
fn test_agent_participant_alignment() {
    let participant = AgentParticipant {
        id: "sub-123".to_string(),
        name: "Security Analyzer".to_string(),
        role: Some("Auditor".to_string()),
        icon: Some("🛡️".to_string()),
        kind: AgentParticipantKind::Subagent,
    };

    let val = serde_json::to_value(&participant).unwrap();
    assert_eq!(val["id"], "sub-123");
    assert_eq!(val["name"], "Security Analyzer");
    assert_eq!(val["role"], "Auditor");
    assert_eq!(val["icon"], "🛡️");
    assert_eq!(val["type"], "subagent");

    let deserialized: AgentParticipant = serde_json::from_value(val).unwrap();
    assert_eq!(deserialized, participant);
}

#[test]
fn test_agent_memory_finding_alignment() {
    let ts = Utc.with_ymd_and_hms(2026, 9, 24, 12, 0, 0).unwrap();
    let finding = AgentMemoryFinding {
        id: "find-1".to_string(),
        summary: "Buffer overflow risk detected".to_string(),
        category: Some("vulnerability".to_string()),
        source_tool: Some("core.code.analyze".to_string()),
        timestamp: ts,
    };

    let val = serde_json::to_value(&finding).unwrap();
    assert_eq!(val["id"], "find-1");
    assert_eq!(val["summary"], "Buffer overflow risk detected");
    assert_eq!(val["category"], "vulnerability");
    assert_eq!(val["sourceTool"], "core.code.analyze");
    assert_eq!(val["timestamp"], "2026-09-24T12:00:00Z");

    let from_ts: AgentMemoryFinding = serde_json::from_value(json!({
        "id": "find-2",
        "summary": "Verified database migration success",
        "timestamp": "2026-09-24T12:30:00Z"
    }))
    .unwrap();
    assert_eq!(from_ts.id, "find-2");
    assert_eq!(from_ts.category, None);
    assert_eq!(from_ts.source_tool, None);
}

#[test]
fn test_agent_message_envelope_alignment() {
    let ts = Utc.with_ymd_and_hms(2026, 9, 24, 12, 0, 0).unwrap();
    let envelope = AgentMessageEnvelope {
        id: "env-100".to_string(),
        conversation_id: "conv-42".to_string(),
        parent_message_id: Some("env-99".to_string()),
        correlation_id: Some("corr-7".to_string()),
        sender: AgentParticipant::orchestrator("orch-1", "Lead Orchestrator"),
        recipient: AgentParticipant::subagent("sub-2", "Worker Specialist", "worker", None),
        message_type: AgentMessageType::TaskDelegation,
        payload: AgentMessagePayload {
            content: "Please audit the authorization guard".to_string(),
            structured_data: Some(json!({ "depth": 3 })),
            artifacts: vec![AgentArtifactRef {
                id: "art-1".to_string(),
                title: "AST Dump".to_string(),
                kind: Some("ast".to_string()),
                uri: Some("file:///tmp/ast.json".to_string()),
            }],
            suggested_actions: vec!["run_tests".to_string()],
        },
        permission_profile_id: Some("perm-std".to_string()),
        priority: Some(AgentRunPriority::High),
        timestamp: ts,
    };

    let json_val = serde_json::to_value(&envelope).unwrap();
    assert_eq!(json_val["id"], "env-100");
    assert_eq!(json_val["conversationId"], "conv-42");
    assert_eq!(json_val["parentMessageId"], "env-99");
    assert_eq!(json_val["correlationId"], "corr-7");
    assert_eq!(json_val["sender"]["type"], "orchestrator");
    assert_eq!(json_val["recipient"]["type"], "subagent");
    assert_eq!(json_val["messageType"], "task_delegation");
    assert_eq!(
        json_val["payload"]["content"],
        "Please audit the authorization guard"
    );
    assert_eq!(json_val["payload"]["artifacts"][0]["title"], "AST Dump");
    assert_eq!(json_val["priority"], "high");
    assert_eq!(json_val["timestamp"], "2026-09-24T12:00:00Z");

    let deserialized: AgentMessageEnvelope = serde_json::from_value(json_val).unwrap();
    assert_eq!(deserialized, envelope);
}

#[test]
fn test_agent_memory_context_alignment() {
    let ts = Utc.with_ymd_and_hms(2026, 9, 24, 12, 0, 0).unwrap();
    let mut context = AgentMemoryContext::new("conv-1", "agent-1", "Test Agent", "specialist");
    context.scratchpad = "Step 1 complete, proceeding to step 2.".to_string();
    context.updated_at = ts;
    context.findings.push(AgentMemoryFinding {
        id: "f1".to_string(),
        summary: "All schemas aligned".to_string(),
        category: Some("fact".to_string()),
        source_tool: None,
        timestamp: ts,
    });

    let json_val = serde_json::to_value(&context).unwrap();
    assert_eq!(json_val["agentId"], "agent-1");
    assert_eq!(json_val["agentName"], "Test Agent");
    assert_eq!(json_val["role"], "specialist");
    assert_eq!(json_val["conversationId"], "conv-1");
    assert_eq!(
        json_val["scratchpad"],
        "Step 1 complete, proceeding to step 2."
    );
    assert_eq!(json_val["findings"].as_array().unwrap().len(), 1);
    assert_eq!(json_val["updatedAt"], "2026-09-24T12:00:00Z");

    // Test deserialization from TypeScript contracts style JSON
    let ts_payload = json!({
        "agentId": "sub-99",
        "agentName": "Worker",
        "role": "executor",
        "conversationId": "conv-99",
        "scratchpad": "working on task",
        "findings": [],
        "ledger": [],
        "artifacts": [],
        "updatedAt": "2026-09-24T12:45:00.000Z"
    });

    let parsed: AgentMemoryContext = serde_json::from_value(ts_payload).unwrap();
    assert_eq!(parsed.agent_id, "sub-99");
    assert_eq!(parsed.agent_name, "Worker");
    assert_eq!(parsed.role, "executor");
    assert_eq!(parsed.scratchpad, "working on task");
    assert!(parsed.findings.is_empty());
}
