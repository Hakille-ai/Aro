use serde::{Deserialize, Serialize};

pub trait LifecycleState {
    fn is_terminal(&self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Draft,
    Active,
    Suspended,
    Archived,
}

impl LifecycleState for AgentState {
    fn is_terminal(&self) -> bool {
        matches!(*self, Self::Archived)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Created,
    Queued,
    Starting,
    Running,
    WaitingForTool,
    WaitingForEvent,
    WaitingForUser,
    WaitingForResource,
    Paused,
    Retrying,
    Recovering,
    Completing,
    Completed,
    Failed,
    Cancelled,
    Expired,
}

impl LifecycleState for RunState {
    fn is_terminal(&self) -> bool {
        matches!(
            *self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Expired
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Draft,
    Blocked,
    Ready,
    Claimed,
    Running,
    Waiting,
    Paused,
    RetryWait,
    Succeeded,
    Failed,
    Cancelled,
    Skipped,
    Compensated,
}

impl LifecycleState for TaskState {
    fn is_terminal(&self) -> bool {
        matches!(
            *self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Skipped | Self::Compensated
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Planned,
    Ready,
    Executing,
    Waiting,
    Succeeded,
    Failed,
    Cancelled,
    Skipped,
}

impl LifecycleState for StepState {
    fn is_terminal(&self) -> bool {
        matches!(
            *self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Skipped
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepAttemptState {
    Leased,
    Started,
    EffectPrepared,
    EffectUnknown,
    EffectCommitted,
    ResultRecorded,
    Failed,
    Abandoned,
}

impl StepAttemptState {
    #[must_use]
    pub const fn effect_may_have_happened(self) -> bool {
        matches!(
            self,
            Self::EffectPrepared
                | Self::EffectUnknown
                | Self::EffectCommitted
                | Self::ResultRecorded
        )
    }
}

impl LifecycleState for StepAttemptState {
    fn is_terminal(&self) -> bool {
        matches!(*self, Self::ResultRecorded | Self::Failed | Self::Abandoned)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    Requested,
    Granted,
    Rejected,
    Expired,
    Revoked,
    Consumed,
}

impl LifecycleState for ApprovalState {
    fn is_terminal(&self) -> bool {
        matches!(
            *self,
            Self::Rejected | Self::Expired | Self::Revoked | Self::Consumed
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentState {
    Requested,
    Provisioning,
    Ready,
    Busy,
    Suspended,
    Draining,
    Destroying,
    Destroyed,
    Failed,
    Quarantined,
}

impl LifecycleState for EnvironmentState {
    fn is_terminal(&self) -> bool {
        matches!(*self, Self::Destroyed)
    }
}
