use std::collections::{BTreeSet, HashMap};

use aro_agent_domain::*;
use chrono::{Duration, TimeZone, Utc};
use proptest::prelude::*;
use serde_json::json;
use uuid::Uuid;

fn uuid(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn now(offset_seconds: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1_750_000_000 + offset_seconds, 0)
        .single()
        .expect("test timestamp is valid")
}

fn tenant(value: u128) -> TenantId {
    TenantId::from_uuid(uuid(value))
}

fn command<C>(
    tenant_id: TenantId,
    version: AggregateVersion,
    payload: C,
    sequence: u128,
) -> CommandEnvelope<C> {
    CommandEnvelope {
        command_id: CommandId::from_uuid(uuid(10_000 + sequence)),
        idempotency_key: IdempotencyKey::parse(format!("test-{sequence}"))
            .expect("test key is valid"),
        tenant_id,
        actor: ActorRef::User(UserId::from_uuid(uuid(50))),
        expected_version: version,
        requested_at: now(sequence as i64),
        correlation_id: CorrelationId::from_uuid(uuid(100)),
        causation_id: None,
        payload,
    }
}

fn transition(sequence: u128) -> TransitionContext {
    TransitionContext {
        event_id: EventId::from_uuid(uuid(20_000 + sequence)),
        recorded_at: now(sequence as i64) + Duration::milliseconds(25),
    }
}

#[test]
fn canonical_json_is_stable_across_object_insertion_order() {
    let mut left = HashMap::new();
    left.insert("z", json!({"b": 2, "a": 1}));
    left.insert("a", json!([3, 2, 1]));

    let mut right = HashMap::new();
    right.insert("a", json!([3, 2, 1]));
    right.insert("z", json!({"a": 1, "b": 2}));

    let left_bytes = canonical_json_bytes(&left).expect("canonical JSON");
    let right_bytes = canonical_json_bytes(&right).expect("canonical JSON");
    assert_eq!(left_bytes, right_bytes);
    assert_eq!(
        canonical_json_digest(&left).expect("digest"),
        canonical_json_digest(&right).expect("digest")
    );
    assert_eq!(
        String::from_utf8(left_bytes).expect("UTF-8"),
        r#"{"a":[3,2,1],"z":{"a":1,"b":2}}"#
    );
}

#[test]
fn typed_keys_and_digests_reject_ambiguous_values() {
    assert_eq!(AgentId::new().into_uuid().get_version_num(), 7);
    assert!(IdempotencyKey::parse("").is_err());
    assert!(IdempotencyKey::parse("contains a space").is_err());
    assert!(ContentDigest::parse("ABC").is_err());
    assert!(ContentDigest::parse("A".repeat(64)).is_err());
    assert!(ContentDigest::parse("a".repeat(64)).is_ok());
}

#[test]
fn command_and_event_json_contracts_are_stable_and_camel_cased() {
    let tenant_id = tenant(1);
    let run = RunAggregate::new(RunId::from_uuid(uuid(900)), tenant_id, now(0));
    let command = command(
        tenant_id,
        run.metadata.version,
        RunCommand::Pause {
            safe_boundary: true,
        },
        1,
    );
    let command_json = serde_json::to_value(&command).expect("command serialization");
    assert_eq!(command_json["payload"]["type"], "pause");
    assert_eq!(command_json["payload"]["safeBoundary"], true);
    assert!(command_json["expectedVersion"].is_number());
    assert!(command_json.get("expected_version").is_none());

    let event = AgentEvent::state_change(
        command.event_metadata(&transition(1)),
        SubjectRef::Run(run.id),
        AggregateVersion::new(1),
        "run.paused",
        DomainEvent::RunStateChanged {
            from: RunState::Running,
            to: RunState::Paused,
            reason_code: "paused".into(),
        },
    );
    let event_json = serde_json::to_value(&event).expect("event serialization");
    assert_eq!(event_json["eventVersion"], 1);
    assert_eq!(event_json["payload"]["type"], "run_state_changed");
    assert_eq!(event_json["payload"]["reasonCode"], "paused");
    assert_eq!(
        serde_json::to_value(RunState::WaitingForUser).expect("state serialization"),
        json!("waiting_for_user")
    );

    let round_trip: AgentEvent = serde_json::from_value(event_json).expect("event deserialization");
    assert_eq!(round_trip, event);
}

#[test]
fn agent_version_spec_rejects_implicit_cross_locality_fallback() {
    let mut allowed_kinds = BTreeSet::new();
    allowed_kinds.insert(EnvironmentKind::Sandbox);
    let spec = AgentVersionSpec {
        schema_version: AgentVersionSpec::CURRENT_SCHEMA_VERSION,
        display_name: "Research agent".into(),
        objective: "Produce evidence-backed research".into(),
        instructions_ref: "object://instructions/1".into(),
        instructions_digest: ContentDigest::from_bytes(b"instructions"),
        autonomy: AutonomyLevel::ConfirmSensitive,
        model_policy: ModelRoutingPolicy {
            primary: ModelRouteSpec {
                provider_id: "local".into(),
                model_id: "reasoner-v1".into(),
                locality: ExecutionLocality::Local,
                region: None,
                maximum_data_classification: DataClassification::Restricted,
            },
            fallbacks: vec![ModelRouteSpec {
                provider_id: "cloud".into(),
                model_id: "reasoner-v2".into(),
                locality: ExecutionLocality::PublicCloud,
                region: Some("eu-west".into()),
                maximum_data_classification: DataClassification::Internal,
            }],
            allow_cross_locality_fallback: false,
        },
        capability_policy: CapabilityPolicy::default(),
        memory_policy: MemoryPolicy {
            readable_scopes: BTreeSet::new(),
            write_mode: MemoryWriteMode::ProposeOnly,
        },
        environment_policy: EnvironmentPolicy {
            default_profile_id: Some(EnvironmentProfileId::from_uuid(uuid(500))),
            allowed_kinds,
            can_change_environment: false,
        },
        runtime_limits: RuntimeLimits {
            maximum_tasks: 20,
            maximum_parallel_tasks: 4,
            maximum_steps_per_task: 50,
            maximum_subagent_depth: 2,
            maximum_subagents: 6,
            activation_timeout_seconds: 300,
            run_timeout_seconds: 86_400,
        },
        budget_limits: BudgetLimits {
            cost_microunits: 10_000_000,
            input_tokens: 1_000_000,
            output_tokens: 250_000,
            tool_calls: 500,
            model_calls: 200,
        },
    };

    assert!(matches!(
        spec.validate(),
        Err(AgentDomainError::InvalidAgentSpec(message))
            if message.contains("cross-locality")
    ));
    let mut permitted = spec;
    permitted.model_policy.allow_cross_locality_fallback = true;
    permitted.validate().expect("explicit policy is valid");
}

#[test]
fn run_lifecycle_is_versioned_causal_and_terminal() {
    let tenant_id = tenant(1);
    let mut run = RunAggregate::new(RunId::from_uuid(uuid(1_000)), tenant_id, now(0));
    let sequence = [
        RunCommand::Queue,
        RunCommand::Claim,
        RunCommand::Start,
        RunCommand::WaitForEvent,
        RunCommand::Wake,
        RunCommand::Claim,
        RunCommand::Start,
        RunCommand::BeginCompletion,
        RunCommand::Complete {
            result_id: ResultId::from_uuid(uuid(2_000)),
        },
    ];

    for (index, payload) in sequence.into_iter().enumerate() {
        let number = index as u128 + 1;
        let applied = reduce_run(
            &run,
            &command(tenant_id, run.metadata.version, payload, number),
            &transition(number),
        )
        .expect("valid run transition");
        assert_eq!(
            applied.event.subject_version,
            AggregateVersion::new(number as u64)
        );
        assert_eq!(applied.event.tenant_id, tenant_id);
        assert!(matches!(applied.event.subject, SubjectRef::Run(id) if id == run.id));
        assert!(applied.event.recorded_at >= applied.event.occurred_at);
        run = applied.aggregate;
    }

    assert_eq!(run.status, RunState::Completed);
    assert_eq!(run.metadata.version, AggregateVersion::new(9));
    assert!(run.final_result_id.is_some());
    let invalid = reduce_run(
        &run,
        &command(tenant_id, run.metadata.version, RunCommand::Resume, 20),
        &transition(20),
    );
    assert!(matches!(
        invalid,
        Err(AgentDomainError::InvalidTransition { .. })
    ));
}

#[test]
fn active_runs_require_a_safe_boundary_for_pause_cancel_and_expiry() {
    let tenant_id = tenant(1);
    let mut run = RunAggregate::new(RunId::from_uuid(uuid(1_001)), tenant_id, now(0));
    for (sequence, payload) in [RunCommand::Queue, RunCommand::Claim, RunCommand::Start]
        .into_iter()
        .enumerate()
    {
        let number = sequence as u128 + 1;
        run = reduce_run(
            &run,
            &command(tenant_id, run.metadata.version, payload, number),
            &transition(number),
        )
        .expect("valid transition")
        .aggregate;
    }

    for payload in [
        RunCommand::Pause {
            safe_boundary: false,
        },
        RunCommand::Cancel {
            safe_boundary: false,
        },
        RunCommand::Expire {
            safe_boundary: false,
        },
    ] {
        assert_eq!(
            reduce_run(
                &run,
                &command(tenant_id, run.metadata.version, payload, 10),
                &transition(10),
            )
            .expect_err("unsafe transition"),
            AgentDomainError::UnsafeBoundary
        );
    }
}

#[test]
fn tenant_and_optimistic_version_are_checked_before_transition() {
    let tenant_id = tenant(1);
    let run = RunAggregate::new(RunId::from_uuid(uuid(1_002)), tenant_id, now(0));
    let wrong_tenant = reduce_run(
        &run,
        &command(tenant(2), run.metadata.version, RunCommand::Queue, 1),
        &transition(1),
    );
    assert!(matches!(
        wrong_tenant,
        Err(AgentDomainError::InvariantViolation(_))
    ));

    let wrong_version = reduce_run(
        &run,
        &command(tenant_id, AggregateVersion::new(4), RunCommand::Queue, 2),
        &transition(2),
    );
    assert_eq!(
        wrong_version.expect_err("version conflict"),
        AgentDomainError::VersionConflict {
            expected: 4,
            actual: 0
        }
    );
}

#[test]
fn task_step_and_attempt_keep_results_and_effect_boundaries_distinct() {
    let tenant_id = tenant(1);
    let run_id = RunId::from_uuid(uuid(3_000));
    let mut task = TaskAggregate::new(
        TaskId::from_uuid(uuid(3_001)),
        run_id,
        None,
        tenant_id,
        now(0),
    );
    for (index, payload) in [
        TaskCommand::MarkReady,
        TaskCommand::Claim,
        TaskCommand::Start,
        TaskCommand::Succeed {
            result_id: ResultId::from_uuid(uuid(3_002)),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let number = index as u128 + 1;
        task = reduce_task(
            &task,
            &command(tenant_id, task.metadata.version, payload, number),
            &transition(number),
        )
        .expect("task transition")
        .aggregate;
    }
    assert_eq!(task.status, TaskState::Succeeded);
    assert!(task.result_id.is_some());

    let mut step = StepAggregate::new(StepId::from_uuid(uuid(3_010)), task.id, tenant_id, now(0));
    for (index, payload) in [StepCommand::MarkReady, StepCommand::Start]
        .into_iter()
        .enumerate()
    {
        let number = index as u128 + 10;
        step = reduce_step(
            &step,
            &command(tenant_id, step.metadata.version, payload, number),
            &transition(number),
        )
        .expect("step transition")
        .aggregate;
    }

    let mut attempt = StepAttemptAggregate::new(
        StepAttemptId::from_uuid(uuid(3_020)),
        step.id,
        1,
        tenant_id,
        now(0),
    );
    let effect_key = ContentDigest::from_bytes(b"stable external effect");
    for (index, payload) in [
        StepAttemptCommand::Start,
        StepAttemptCommand::PrepareEffect {
            effect_key: effect_key.clone(),
        },
        StepAttemptCommand::CommitEffect {
            external_effect_ref: "provider-operation-42".into(),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let number = index as u128 + 20;
        attempt = reduce_step_attempt(
            &attempt,
            &command(tenant_id, attempt.metadata.version, payload, number),
            &transition(number),
        )
        .expect("attempt transition")
        .aggregate;
    }
    assert!(attempt.status.effect_may_have_happened());
    assert_eq!(attempt.effect_key, Some(effect_key));
    assert!(reduce_step_attempt(
        &attempt,
        &command(
            tenant_id,
            attempt.metadata.version,
            StepAttemptCommand::Fail {
                error_code: "provider.timeout".into(),
            },
            30,
        ),
        &transition(30),
    )
    .is_err());

    attempt = reduce_step_attempt(
        &attempt,
        &command(
            tenant_id,
            attempt.metadata.version,
            StepAttemptCommand::RecordResult {
                result_id: ResultId::from_uuid(uuid(3_021)),
            },
            31,
        ),
        &transition(31),
    )
    .expect("committed effect is reconciled by recording its result")
    .aggregate;
    assert_eq!(attempt.status, StepAttemptState::ResultRecorded);
}

#[test]
fn approval_is_expiring_single_use_and_bound_to_the_exact_action_digest() {
    let tenant_id = tenant(1);
    let digest = ContentDigest::from_bytes(b"canonical action");
    let mut approval = ApprovalAggregate::new(
        ApprovalId::from_uuid(uuid(4_000)),
        tenant_id,
        digest.clone(),
        now(100),
        now(0),
    );

    let mismatch = reduce_approval(
        &approval,
        &command(
            tenant_id,
            approval.metadata.version,
            ApprovalCommand::Grant {
                action_digest: ContentDigest::from_bytes(b"modified action"),
            },
            1,
        ),
        &transition(1),
    );
    assert_eq!(
        mismatch.expect_err("digest mismatch"),
        AgentDomainError::ApprovalDigestMismatch
    );

    approval = reduce_approval(
        &approval,
        &command(
            tenant_id,
            approval.metadata.version,
            ApprovalCommand::Grant {
                action_digest: digest.clone(),
            },
            2,
        ),
        &transition(2),
    )
    .expect("approval granted")
    .aggregate;
    approval = reduce_approval(
        &approval,
        &command(
            tenant_id,
            approval.metadata.version,
            ApprovalCommand::Consume {
                action_digest: digest,
            },
            3,
        ),
        &transition(3),
    )
    .expect("approval consumed")
    .aggregate;
    assert_eq!(approval.status, ApprovalState::Consumed);
    assert!(reduce_approval(
        &approval,
        &command(
            tenant_id,
            approval.metadata.version,
            ApprovalCommand::Revoke,
            4,
        ),
        &transition(4),
    )
    .is_err());
}

#[test]
fn environment_cannot_be_destroyed_without_a_cleanup_receipt() {
    let tenant_id = tenant(1);
    let mut environment =
        EnvironmentAggregate::new(EnvironmentId::from_uuid(uuid(5_000)), tenant_id, now(0));
    for (index, payload) in [
        EnvironmentCommand::BeginProvisioning,
        EnvironmentCommand::MarkReady,
        EnvironmentCommand::BeginDrain {
            safe_boundary: true,
        },
        EnvironmentCommand::BeginDestroy,
        EnvironmentCommand::MarkDestroyed {
            cleanup_receipt_artifact_id: ArtifactId::from_uuid(uuid(5_001)),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let number = index as u128 + 1;
        environment = reduce_environment(
            &environment,
            &command(tenant_id, environment.metadata.version, payload, number),
            &transition(number),
        )
        .expect("environment transition")
        .aggregate;
    }
    assert_eq!(environment.status, EnvironmentState::Destroyed);
    assert!(environment.cleanup_receipt_artifact_id.is_some());
    assert!(reduce_environment(
        &environment,
        &command(
            tenant_id,
            environment.metadata.version,
            EnvironmentCommand::BeginProvisioning,
            10,
        ),
        &transition(10),
    )
    .is_err());
}

#[test]
fn prepared_effect_with_a_lost_lease_requires_reconciliation_before_retry() {
    let tenant_id = tenant(1);
    let mut attempt = StepAttemptAggregate::new(
        StepAttemptId::from_uuid(uuid(6_000)),
        StepId::from_uuid(uuid(6_001)),
        1,
        tenant_id,
        now(0),
    );
    for (index, payload) in [
        StepAttemptCommand::Start,
        StepAttemptCommand::PrepareEffect {
            effect_key: ContentDigest::from_bytes(b"possibly committed effect"),
        },
        StepAttemptCommand::Abandon,
    ]
    .into_iter()
    .enumerate()
    {
        let number = index as u128 + 1;
        attempt = reduce_step_attempt(
            &attempt,
            &command(tenant_id, attempt.metadata.version, payload, number),
            &transition(number),
        )
        .expect("attempt transition")
        .aggregate;
    }

    assert_eq!(attempt.status, StepAttemptState::EffectUnknown);
    assert!(attempt.status.effect_may_have_happened());
    assert!(reduce_step_attempt(
        &attempt,
        &command(
            tenant_id,
            attempt.metadata.version,
            StepAttemptCommand::Start,
            10,
        ),
        &transition(10),
    )
    .is_err());

    attempt = reduce_step_attempt(
        &attempt,
        &command(
            tenant_id,
            attempt.metadata.version,
            StepAttemptCommand::ReconcileEffectAbsent,
            11,
        ),
        &transition(11),
    )
    .expect("provider proved the effect did not happen")
    .aggregate;
    assert_eq!(attempt.status, StepAttemptState::Failed);
}

fn terminal_run_state() -> impl Strategy<Value = RunState> {
    prop_oneof![
        Just(RunState::Completed),
        Just(RunState::Failed),
        Just(RunState::Cancelled),
        Just(RunState::Expired),
    ]
}

fn arbitrary_run_command() -> impl Strategy<Value = RunCommand> {
    (0_u8..19).prop_map(|value| match value {
        0 => RunCommand::Queue,
        1 => RunCommand::Claim,
        2 => RunCommand::Start,
        3 => RunCommand::WaitForTool,
        4 => RunCommand::WaitForEvent,
        5 => RunCommand::WaitForUser,
        6 => RunCommand::WaitForResource,
        7 => RunCommand::Wake,
        8 => RunCommand::Pause {
            safe_boundary: true,
        },
        9 => RunCommand::Resume,
        10 => RunCommand::ScheduleRetry,
        11 => RunCommand::RetryReady,
        12 => RunCommand::BeginRecovery,
        13 => RunCommand::RecoveryReady,
        14 => RunCommand::BeginCompletion,
        15 => RunCommand::Complete {
            result_id: ResultId::from_uuid(uuid(9_000)),
        },
        16 => RunCommand::Fail {
            error_code: "system.failure".into(),
        },
        17 => RunCommand::Cancel {
            safe_boundary: true,
        },
        _ => RunCommand::Expire {
            safe_boundary: true,
        },
    })
}

proptest! {
    #[test]
    fn no_command_resurrects_a_terminal_run(
        terminal_state in terminal_run_state(),
        payload in arbitrary_run_command(),
    ) {
        let tenant_id = tenant(1);
        let mut run = RunAggregate::new(RunId::from_uuid(uuid(9_100)), tenant_id, now(0));
        run.status = terminal_state;
        let result = reduce_run(
            &run,
            &command(tenant_id, run.metadata.version, payload, 1),
            &transition(1),
        );
        let rejected_as_invalid_transition =
            matches!(result, Err(AgentDomainError::InvalidTransition { .. }));
        prop_assert!(rejected_as_invalid_transition);
    }
}
