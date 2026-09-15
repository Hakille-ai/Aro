use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    ActorRef, AggregateVersion, ArtifactId, CommandId, ContentDigest, CorrelationId, EventId,
    EventMetadata, IdempotencyKey, ResultId, TenantId, TransitionContext,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEnvelope<C> {
    pub command_id: CommandId,
    pub idempotency_key: IdempotencyKey,
    pub tenant_id: TenantId,
    pub actor: ActorRef,
    pub expected_version: AggregateVersion,
    pub requested_at: DateTime<Utc>,
    pub correlation_id: CorrelationId,
    pub causation_id: Option<EventId>,
    pub payload: C,
}

impl<C> CommandEnvelope<C> {
    #[must_use]
    pub fn event_metadata(&self, transition: &TransitionContext) -> EventMetadata {
        EventMetadata {
            event_id: transition.event_id,
            command_id: self.command_id,
            tenant_id: self.tenant_id,
            actor: self.actor.clone(),
            expected_version: self.expected_version,
            occurred_at: self.requested_at,
            recorded_at: transition.recorded_at,
            correlation_id: self.correlation_id,
            causation_id: self.causation_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum RunCommand {
    Queue,
    Claim,
    Start,
    WaitForTool,
    WaitForEvent,
    WaitForUser,
    WaitForResource,
    Wake,
    Pause { safe_boundary: bool },
    Resume,
    ScheduleRetry,
    RetryReady,
    BeginRecovery,
    RecoveryReady,
    BeginCompletion,
    Complete { result_id: ResultId },
    Fail { error_code: String },
    Cancel { safe_boundary: bool },
    Expire { safe_boundary: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum TaskCommand {
    Block,
    MarkReady,
    Claim,
    Start,
    Wait,
    Wake,
    Pause {
        safe_boundary: bool,
    },
    Resume,
    Succeed {
        result_id: ResultId,
    },
    Fail {
        error_code: String,
        retry_scheduled: bool,
    },
    Cancel {
        safe_boundary: bool,
    },
    Skip,
    MarkCompensated {
        result_id: Option<ResultId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum StepCommand {
    MarkReady,
    Start,
    Wait,
    Wake,
    Succeed { result_id: ResultId },
    Fail { error_code: String },
    Cancel { safe_boundary: bool },
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum StepAttemptCommand {
    Start,
    PrepareEffect { effect_key: ContentDigest },
    CommitEffect { external_effect_ref: String },
    ReconcileEffectCommitted { external_effect_ref: String },
    ReconcileEffectAbsent,
    RecordResult { result_id: ResultId },
    Fail { error_code: String },
    Abandon,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ApprovalCommand {
    Grant { action_digest: ContentDigest },
    Reject { action_digest: ContentDigest },
    Expire,
    Revoke,
    Consume { action_digest: ContentDigest },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum EnvironmentCommand {
    BeginProvisioning,
    MarkReady,
    Acquire,
    Release,
    Suspend {
        safe_boundary: bool,
    },
    Resume,
    BeginDrain {
        safe_boundary: bool,
    },
    BeginDestroy,
    MarkDestroyed {
        cleanup_receipt_artifact_id: ArtifactId,
    },
    Fail {
        error_code: String,
    },
    Quarantine {
        error_code: String,
    },
}
