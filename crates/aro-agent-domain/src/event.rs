use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    AgentId, AggregateVersion, ApprovalId, ApprovalState, CommandId, CorrelationId, EnvironmentId,
    EnvironmentState, EventId, RunId, RunState, StepAttemptId, StepAttemptState, StepId, StepState,
    TaskId, TaskState, TenantId, UserId,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "snake_case")]
pub enum ActorRef {
    User(UserId),
    Agent(AgentId),
    System(String),
    Service(String),
}

impl ActorRef {
    #[must_use]
    pub fn is_privileged_system_actor(&self) -> bool {
        matches!(self, Self::System(_) | Self::Service(_))
    }

    pub fn validate(&self) -> Result<(), crate::AgentDomainError> {
        let value = match self {
            Self::System(value) | Self::Service(value) => Some(value),
            Self::User(_) | Self::Agent(_) => None,
        };
        if value.is_some_and(|value| {
            value.is_empty()
                || value.len() > 120
                || !value.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || matches!(character, '.' | '_' | '-')
                })
        }) {
            return Err(crate::AgentDomainError::InvariantViolation(
                "system and service actors require a lowercase machine identifier".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "snake_case")]
pub enum SubjectRef {
    Run(RunId),
    Task(TaskId),
    Step(StepId),
    StepAttempt(StepAttemptId),
    Approval(ApprovalId),
    Environment(EnvironmentId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventMetadata {
    pub event_id: EventId,
    pub command_id: CommandId,
    pub tenant_id: TenantId,
    pub actor: ActorRef,
    pub expected_version: AggregateVersion,
    pub occurred_at: DateTime<Utc>,
    pub recorded_at: DateTime<Utc>,
    pub correlation_id: CorrelationId,
    pub causation_id: Option<EventId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionContext {
    pub event_id: EventId,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum DomainEvent {
    RunStateChanged {
        from: RunState,
        to: RunState,
        reason_code: String,
    },
    TaskStateChanged {
        from: TaskState,
        to: TaskState,
        reason_code: String,
    },
    StepStateChanged {
        from: StepState,
        to: StepState,
        reason_code: String,
    },
    StepAttemptStateChanged {
        from: StepAttemptState,
        to: StepAttemptState,
        reason_code: String,
    },
    ApprovalStateChanged {
        from: ApprovalState,
        to: ApprovalState,
        reason_code: String,
    },
    EnvironmentStateChanged {
        from: EnvironmentState,
        to: EnvironmentState,
        reason_code: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    pub event_id: EventId,
    pub event_type: String,
    pub event_version: u16,
    pub tenant_id: TenantId,
    pub actor: ActorRef,
    pub subject: SubjectRef,
    pub subject_version: AggregateVersion,
    pub command_id: CommandId,
    pub correlation_id: CorrelationId,
    pub causation_id: Option<EventId>,
    pub occurred_at: DateTime<Utc>,
    pub recorded_at: DateTime<Utc>,
    pub payload_schema: String,
    pub payload: DomainEvent,
}

impl AgentEvent {
    pub const EVENT_VERSION: u16 = 1;
    pub const PAYLOAD_SCHEMA: &'static str = "aro.agent.state-change.v1";

    #[must_use]
    pub fn state_change(
        metadata: EventMetadata,
        subject: SubjectRef,
        subject_version: AggregateVersion,
        event_type: impl Into<String>,
        payload: DomainEvent,
    ) -> Self {
        Self {
            event_id: metadata.event_id,
            event_type: event_type.into(),
            event_version: Self::EVENT_VERSION,
            tenant_id: metadata.tenant_id,
            actor: metadata.actor,
            subject,
            subject_version,
            command_id: metadata.command_id,
            correlation_id: metadata.correlation_id,
            causation_id: metadata.causation_id,
            occurred_at: metadata.occurred_at,
            recorded_at: metadata.recorded_at,
            payload_schema: Self::PAYLOAD_SCHEMA.into(),
            payload,
        }
    }
}
