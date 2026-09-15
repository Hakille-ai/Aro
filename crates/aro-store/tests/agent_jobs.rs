use aro_core::{AgentRun, AgentRunPriority, AgentRunStatus, AssistantMode};
use aro_store::{
    AgentRunJobBudgets, AgentRunJobCompletion, AgentRunJobCompletionOutcome,
    AgentRunJobLeaseRenewal, AgentRunJobRetryOutcome, AgentRunJobStatus, AgentRunJobSubmission,
    AgentRunJobUsage, AgentRunJobWorkerEvent, AgentSnapshotKeyring, AroStore, NewUserWithOrg,
    TenantContext,
};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const SNAPSHOT_KEY: &str = "queue-test-only-snapshot-key-32-bytes-minimum";

#[tokio::test]
async fn durable_agent_jobs_are_idempotent_fenced_and_atomic() -> TestResult {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping durable agent queue test: DATABASE_URL is not set");
        return Ok(());
    };
    let store = AroStore::connect(&database_url, 8).await?;
    let keyring = AgentSnapshotKeyring::single("test-v1", SNAPSHOT_KEY)?;
    let suffix = Uuid::new_v4();
    let principal = store
        .create_user_with_org(NewUserWithOrg {
            email: format!("agent-queue-{suffix}@example.test"),
            name: "Queue test".to_string(),
            role_title: None,
            avatar_color: None,
            password_hash: "not-a-real-password-hash".to_string(),
            organization_name: format!("Queue test {suffix}"),
            organization_domain: None,
            organization_description: None,
        })
        .await?;
    let context = TenantContext::new(principal.user.id, principal.active_organization.id)?;
    let conversation = store
        .create_conversation(
            context,
            "Durable queue test".to_string(),
            AssistantMode::Chat,
        )
        .await?;
    let lane = store
        .ensure_agent_lane(
            principal.user.id,
            principal.active_organization.id,
            Some(conversation.id),
            "Durable queue".to_string(),
        )
        .await?;

    let mut run = AgentRun::new(
        "Prove durable execution",
        AssistantMode::Chat,
        Some(conversation.id),
        Some("test-provider".to_string()),
        Some("test-model".to_string()),
        None,
    );
    run.lane_id = Some(lane.id);
    run.status = AgentRunStatus::Queued;
    run.priority = AgentRunPriority::Critical;
    let snapshot = json!({
        "version": 1,
        "goal": run.goal.clone(),
        "conversationId": conversation.id,
    });
    let created = store
        .submit_agent_run_job(
            context,
            &run,
            "queue-test-primary",
            &snapshot,
            AgentRunJobBudgets::default(),
            &keyring,
        )
        .await?;
    let created = match created {
        AgentRunJobSubmission::Created(job) => job,
        unexpected => panic!("expected a created job, got {unexpected:?}"),
    };
    let replayed = store
        .submit_agent_run_job(
            context,
            &run,
            "queue-test-primary",
            &snapshot,
            AgentRunJobBudgets::default(),
            &keyring,
        )
        .await?;
    match replayed {
        AgentRunJobSubmission::Replayed(job) => assert_eq!(job.id, created.id),
        unexpected => panic!("expected an idempotent replay, got {unexpected:?}"),
    }
    assert_eq!(
        store
            .submit_agent_run_job(
                context,
                &run,
                "queue-test-primary",
                &json!({ "version": 1, "goal": "different request" }),
                AgentRunJobBudgets::default(),
                &keyring,
            )
            .await?,
        AgentRunJobSubmission::Conflict
    );

    let claimed = store
        .claim_agent_run_job("queue-test-worker", 60, &keyring)
        .await?
        .ok_or("the submitted job was not claimable")?;
    assert_eq!(claimed.lease.agent_run_id(), run.id);
    assert_eq!(claimed.request_snapshot["schemaVersion"], 1);
    assert_eq!(claimed.request_snapshot["request"], snapshot);
    assert_eq!(claimed.attempts, 1);
    assert!(matches!(
        store
            .renew_agent_run_job_lease(
                &claimed.lease,
                60,
                AgentRunJobUsage {
                    steps: 1,
                    tool_calls: 1,
                    input_tokens: 100,
                    output_tokens: 10,
                },
            )
            .await?,
        AgentRunJobLeaseRenewal::Renewed { .. }
    ));
    let cancellation = store
        .request_agent_run_job_cancellation(context, run.id)
        .await?;
    assert_eq!(cancellation.status, AgentRunJobStatus::Running);
    assert!(cancellation.cancel_requested_at.is_some());
    assert_eq!(
        store
            .renew_agent_run_job_lease(&claimed.lease, 60, AgentRunJobUsage::default())
            .await?,
        AgentRunJobLeaseRenewal::CancellationRequested
    );
    assert_eq!(
        store
            .release_agent_run_job_for_retry(&claimed.lease, "cancelled", 1)
            .await?,
        AgentRunJobRetryOutcome::Cancelled
    );

    let mut completed_run = AgentRun::new(
        "Complete atomically",
        AssistantMode::Chat,
        Some(conversation.id),
        Some("test-provider".to_string()),
        Some("test-model".to_string()),
        None,
    );
    completed_run.lane_id = Some(lane.id);
    completed_run.status = AgentRunStatus::Queued;
    completed_run.priority = AgentRunPriority::Critical;
    let completed_snapshot = json!({ "version": 1, "goal": completed_run.goal.clone() });
    assert!(matches!(
        store
            .submit_agent_run_job(
                context,
                &completed_run,
                "queue-test-completion",
                &completed_snapshot,
                AgentRunJobBudgets::default(),
                &keyring,
            )
            .await?,
        AgentRunJobSubmission::Created(_)
    ));
    let completed_claim = store
        .claim_agent_run_job("queue-test-worker", 60, &keyring)
        .await?
        .ok_or("the completion job was not claimable")?;
    assert_eq!(completed_claim.lease.agent_run_id(), completed_run.id);
    assert!(store
        .append_durable_agent_run_event(
            &completed_claim.lease,
            AgentRunJobWorkerEvent::ModelStarted { step: 1 },
        )
        .await?
        .is_some());
    let completion = AgentRunJobCompletion {
        content: "A durable answer".to_string(),
        model_id: Some("test-model".to_string()),
        token_estimate: Some(4),
        checkpoint_summary: Some("Completed by the queue test".to_string()),
        usage: AgentRunJobUsage {
            steps: 1,
            tool_calls: 0,
            input_tokens: 20,
            output_tokens: 4,
        },
    };
    let result_message_id = match store
        .complete_agent_run_job(&completed_claim.lease, completion.clone())
        .await?
    {
        AgentRunJobCompletionOutcome::Completed {
            result_message_id: Some(message_id),
        } => message_id,
        unexpected => panic!("expected an atomic completion, got {unexpected:?}"),
    };
    assert_eq!(
        store
            .complete_agent_run_job(&completed_claim.lease, completion)
            .await?,
        AgentRunJobCompletionOutcome::AlreadyCompleted {
            result_message_id: Some(result_message_id)
        }
    );
    assert!(store
        .append_durable_agent_run_event(
            &completed_claim.lease,
            AgentRunJobWorkerEvent::CheckpointSaved { step: 2 },
        )
        .await?
        .is_none());
    let message = sqlx::query(
        r#"
        SELECT content, source_agent_run_id
        FROM messages
        WHERE organization_id = $1 AND id = $2
        "#,
    )
    .bind(principal.active_organization.id)
    .bind(result_message_id)
    .fetch_one(store.pool())
    .await?;
    assert_eq!(message.get::<String, _>("content"), "A durable answer");
    assert_eq!(
        message.get::<Option<Uuid>, _>("source_agent_run_id"),
        Some(completed_run.id)
    );
    let events = store
        .list_durable_agent_run_events(context, completed_run.id, None, 100)
        .await?;
    assert_eq!(
        events.first().map(|event| event.event_type.as_str()),
        Some("job.submitted")
    );
    assert_eq!(
        events.last().map(|event| event.event_type.as_str()),
        Some("job.completed")
    );
    assert!(events
        .windows(2)
        .all(|events| events[0].cursor < events[1].cursor));

    // Cleanup is tenant-scoped test data only and verifies parent cascades remain compatible with
    // the append-only event trigger.
    sqlx::query("DELETE FROM organizations WHERE id = $1")
        .bind(principal.active_organization.id)
        .execute(store.pool())
        .await?;
    Ok(())
}
