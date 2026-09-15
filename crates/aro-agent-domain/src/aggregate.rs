use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    AggregateVersion, ApprovalId, ApprovalState, ArtifactId, CheckpointId, ContentDigest,
    EnvironmentId, EnvironmentState, EventId, PlanVersionId, ResultId, RunId, RunState,
    StepAttemptId, StepAttemptState, StepId, StepState, TaskId, TaskState, TenantId,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateMetadata {
    pub tenant_id: TenantId,
    pub version: AggregateVersion,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_event_id: Option<EventId>,
}

impl AggregateMetadata {
    #[must_use]
    pub fn new(tenant_id: TenantId, now: DateTime<Utc>) -> Self {
        Self {
            tenant_id,
            version: AggregateVersion::INITIAL,
            created_at: now,
            updated_at: now,
            last_event_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAggregate {
    pub id: RunId,
    pub status: RunState,
    pub current_plan_version_id: Option<PlanVersionId>,
    pub current_checkpoint_id: Option<CheckpointId>,
    pub final_result_id: Option<ResultId>,
    pub resumed_from_run_id: Option<RunId>,
    pub metadata: AggregateMetadata,
}

impl RunAggregate {
    #[must_use]
    pub fn new(id: RunId, tenant_id: TenantId, now: DateTime<Utc>) -> Self {
        Self {
            id,
            status: RunState::Created,
            current_plan_version_id: None,
            current_checkpoint_id: None,
            final_result_id: None,
            resumed_from_run_id: None,
            metadata: AggregateMetadata::new(tenant_id, now),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAggregate {
    pub id: TaskId,
    pub run_id: RunId,
    pub parent_task_id: Option<TaskId>,
    pub status: TaskState,
    pub result_id: Option<ResultId>,
    pub compensation_result_id: Option<ResultId>,
    pub metadata: AggregateMetadata,
}

impl TaskAggregate {
    #[must_use]
    pub fn new(
        id: TaskId,
        run_id: RunId,
        parent_task_id: Option<TaskId>,
        tenant_id: TenantId,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            run_id,
            parent_task_id,
            status: TaskState::Draft,
            result_id: None,
            compensation_result_id: None,
            metadata: AggregateMetadata::new(tenant_id, now),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepAggregate {
    pub id: StepId,
    pub task_id: TaskId,
    pub status: StepState,
    pub result_id: Option<ResultId>,
    pub metadata: AggregateMetadata,
}

impl StepAggregate {
    #[must_use]
    pub fn new(id: StepId, task_id: TaskId, tenant_id: TenantId, now: DateTime<Utc>) -> Self {
        Self {
            id,
            task_id,
            status: StepState::Planned,
            result_id: None,
            metadata: AggregateMetadata::new(tenant_id, now),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepAttemptAggregate {
    pub id: StepAttemptId,
    pub step_id: StepId,
    pub attempt_number: u32,
    pub status: StepAttemptState,
    pub effect_key: Option<ContentDigest>,
    pub external_effect_ref: Option<String>,
    pub result_id: Option<ResultId>,
    pub metadata: AggregateMetadata,
}

impl StepAttemptAggregate {
    #[must_use]
    pub fn new(
        id: StepAttemptId,
        step_id: StepId,
        attempt_number: u32,
        tenant_id: TenantId,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            step_id,
            attempt_number,
            status: StepAttemptState::Leased,
            effect_key: None,
            external_effect_ref: None,
            result_id: None,
            metadata: AggregateMetadata::new(tenant_id, now),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalAggregate {
    pub id: ApprovalId,
    pub action_digest: ContentDigest,
    pub status: ApprovalState,
    pub expires_at: DateTime<Utc>,
    pub decision_actor: Option<crate::ActorRef>,
    pub consumed_by: Option<crate::ActorRef>,
    pub metadata: AggregateMetadata,
}

impl ApprovalAggregate {
    #[must_use]
    pub fn new(
        id: ApprovalId,
        tenant_id: TenantId,
        action_digest: ContentDigest,
        expires_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            action_digest,
            status: ApprovalState::Requested,
            expires_at,
            decision_actor: None,
            consumed_by: None,
            metadata: AggregateMetadata::new(tenant_id, now),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentAggregate {
    pub id: EnvironmentId,
    pub status: EnvironmentState,
    pub snapshot_artifact_id: Option<ArtifactId>,
    pub cleanup_receipt_artifact_id: Option<ArtifactId>,
    pub metadata: AggregateMetadata,
}

impl EnvironmentAggregate {
    #[must_use]
    pub fn new(id: EnvironmentId, tenant_id: TenantId, now: DateTime<Utc>) -> Self {
        Self {
            id,
            status: EnvironmentState::Requested,
            snapshot_artifact_id: None,
            cleanup_receipt_artifact_id: None,
            metadata: AggregateMetadata::new(tenant_id, now),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition<A> {
    pub aggregate: A,
    pub event: crate::AgentEvent,
}
