use aro_core::{Plan, TaskStep};
use uuid::Uuid;

#[test]
fn test_old_payload_missing_status_and_error() {
    let raw = r#"{
        "id": "step-1",
        "text": "Initial setup step",
        "completed": false
    }"#;
    let step: TaskStep = serde_json::from_str(raw).expect("should deserialize old payload");
    assert_eq!(step.id, "step-1");
    assert_eq!(step.text, "Initial setup step");
    assert!(!step.completed);
    assert_eq!(step.status, Some("pending".to_string()));
    assert_eq!(step.error, None);
}

#[test]
fn test_old_payload_completed_true_missing_status() {
    let raw = r#"{
        "id": "step-2",
        "text": "Completed without status",
        "completed": true
    }"#;
    let step: TaskStep = serde_json::from_str(raw).expect("should deserialize completed true");
    assert_eq!(step.id, "step-2");
    assert!(step.completed);
    // In Rust TaskStep, missing status defaults to Some("pending") via default_task_status
    assert_eq!(step.status, Some("pending".to_string()));
    assert_eq!(step.error, None);
}

#[test]
fn test_null_error_field() {
    let raw = r#"{
        "id": "step-3",
        "text": "Step with explicit null error",
        "completed": false,
        "status": "in_progress",
        "error": null
    }"#;
    let step: TaskStep = serde_json::from_str(raw).expect("should handle null error");
    assert_eq!(step.id, "step-3");
    assert_eq!(step.status, Some("in_progress".to_string()));
    assert_eq!(step.error, None);
}

#[test]
fn test_null_status_field() {
    let raw = r#"{
        "id": "step-4",
        "text": "Step without state",
        "completed": false,
        "status": null,
        "error": null
    }"#;
    let step: TaskStep = serde_json::from_str(raw).expect("should handle null status");
    assert_eq!(step.id, "step-4");
    assert_eq!(step.status, None);
    assert_eq!(step.error, None);

    // When serialized, null fields should be skipped
    let serialized = serde_json::to_string(&step).unwrap();
    assert!(!serialized.contains("\"status\""));
    assert!(!serialized.contains("\"error\""));
}

#[test]
fn test_malformed_status_strings() {
    // Arbitrary strings are preserved in Option<String>
    for status_val in &["unknown", "custom_state", "", "   ", "error", "completed"] {
        let raw = format!(
            r#"{{"id": "step-m", "text": "Step", "completed": false, "status": "{}"}}"#,
            status_val
        );
        let step: TaskStep = serde_json::from_str(&raw).expect("string status accepted");
        assert_eq!(step.status, Some(status_val.to_string()));
    }

    // Non-string types must fail to deserialize
    let non_string_payloads = [
        r#"{"id": "step-m", "text": "Step", "completed": false, "status": 12345}"#,
        r#"{"id": "step-m", "text": "Step", "completed": false, "status": ["pending"]}"#,
        r#"{"id": "step-m", "text": "Step", "completed": false, "status": {"code": 1}}"#,
        r#"{"id": "step-m", "text": "Step", "completed": false, "status": true}"#,
    ];
    for raw in &non_string_payloads {
        let res: Result<TaskStep, _> = serde_json::from_str(raw);
        assert!(
            res.is_err(),
            "non-string status should be rejected: {}",
            raw
        );
    }
}

#[test]
fn test_large_error_strings() {
    // Test with 1 MB error string (e.g. huge stack trace)
    let large_err = "E".repeat(1_000_000);
    let step = TaskStep {
        id: "step-err".to_string(),
        text: "Failing step with huge error".to_string(),
        completed: false,
        status: Some("error".to_string()),
        error: Some(large_err.clone()),
    };

    let serialized = serde_json::to_string(&step).expect("serialize huge error");
    let deserialized: TaskStep = serde_json::from_str(&serialized).expect("deserialize huge error");
    assert_eq!(deserialized.error.as_deref(), Some(large_err.as_str()));
    assert_eq!(deserialized.error.unwrap().len(), 1_000_000);
}

#[test]
fn test_missing_required_fields_rejected() {
    // Missing id
    let missing_id = r#"{"text": "No ID", "completed": false}"#;
    assert!(serde_json::from_str::<TaskStep>(missing_id).is_err());

    // Missing text
    let missing_text = r#"{"id": "step-x", "completed": false}"#;
    assert!(serde_json::from_str::<TaskStep>(missing_text).is_err());

    // Missing completed
    let missing_completed = r#"{"id": "step-x", "text": "No completed"}"#;
    assert!(serde_json::from_str::<TaskStep>(missing_completed).is_err());
}

#[test]
fn test_extra_unrecognized_fields_tolerated() {
    let raw = r#"{
        "id": "step-extra",
        "text": "Step with forward compatibility fields",
        "completed": true,
        "status": "completed",
        "futureField": 12345,
        "metadata": { "author": "aro-agent", "tags": ["m1", "diff"] }
    }"#;
    let step: TaskStep = serde_json::from_str(raw).expect("extra fields should be tolerated");
    assert_eq!(step.id, "step-extra");
    assert_eq!(step.status, Some("completed".to_string()));
}

#[test]
fn test_plan_roundtrip_with_extended_taskstep() {
    let plan = Plan {
        id: Uuid::new_v4(),
        conversation_id: Uuid::new_v4(),
        title: "Milestone 1 Test Plan".to_string(),
        description: Some("Comprehensive stress test of TaskStep in Plan".to_string()),
        tasks: vec![
            TaskStep {
                id: "1".to_string(),
                text: "Old step with default status".to_string(),
                completed: false,
                status: Some("pending".to_string()),
                error: None,
            },
            TaskStep {
                id: "2".to_string(),
                text: "Step in progress".to_string(),
                completed: false,
                status: Some("in_progress".to_string()),
                error: None,
            },
            TaskStep {
                id: "3".to_string(),
                text: "Completed step".to_string(),
                completed: true,
                status: Some("completed".to_string()),
                error: None,
            },
            TaskStep {
                id: "4".to_string(),
                text: "Failed step with error".to_string(),
                completed: false,
                status: Some("error".to_string()),
                error: Some("Compilation error: exit code 1".to_string()),
            },
        ],
        status: "active".to_string(),
        created_at: "2026-09-04T17:00:00Z".to_string(),
        updated_at: "2026-09-04T17:05:00Z".to_string(),
    };

    let serialized = serde_json::to_string(&plan).expect("should serialize plan");
    let deserialized: Plan = serde_json::from_str(&serialized).expect("should deserialize plan");
    assert_eq!(plan, deserialized);
    assert_eq!(deserialized.tasks.len(), 4);
    assert_eq!(deserialized.tasks[3].status, Some("error".to_string()));
    assert_eq!(
        deserialized.tasks[3].error,
        Some("Compilation error: exit code 1".to_string())
    );
}
