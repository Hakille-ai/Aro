use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::AgentDomainError;

macro_rules! define_uuid_id {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(
                Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
            )]
            #[serde(transparent)]
            pub struct $name(Uuid);

            impl $name {
                #[must_use]
                pub fn new() -> Self {
                    Self(Uuid::now_v7())
                }

                #[must_use]
                pub const fn from_uuid(value: Uuid) -> Self {
                    Self(value)
                }

                #[must_use]
                pub const fn into_uuid(self) -> Uuid {
                    self.0
                }
            }

            impl Default for $name {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.0.fmt(formatter)
                }
            }

            impl From<Uuid> for $name {
                fn from(value: Uuid) -> Self {
                    Self::from_uuid(value)
                }
            }

            impl From<$name> for Uuid {
                fn from(value: $name) -> Self {
                    value.into_uuid()
                }
            }

            impl FromStr for $name {
                type Err = uuid::Error;

                fn from_str(value: &str) -> Result<Self, Self::Err> {
                    Uuid::parse_str(value).map(Self::from_uuid)
                }
            }
        )+
    };
}

define_uuid_id!(
    TenantId,
    UserId,
    AgentTemplateId,
    AgentTemplateVersionId,
    AgentId,
    AgentVersionId,
    AgentInstanceId,
    RunId,
    SessionId,
    ConversationId,
    TaskId,
    StepId,
    StepAttemptId,
    PlanId,
    PlanVersionId,
    CheckpointId,
    EventId,
    CommandId,
    ApprovalId,
    EnvironmentId,
    EnvironmentProfileId,
    WorkspaceId,
    ToolCallId,
    ModelCallId,
    ArtifactId,
    ResultId,
    MemoryId,
    ContextItemId,
    EffectId,
    BudgetAccountId,
    QuotaPolicyId,
    PolicyDecisionId,
    TriggerId,
    ScheduleId,
    DelegationId,
    CorrelationId,
);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    pub const MAX_LENGTH: usize = 200;

    pub fn parse(value: impl Into<String>) -> Result<Self, AgentDomainError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > Self::MAX_LENGTH
            || !value
                .chars()
                .all(|character| character.is_ascii_graphic() && !character.is_ascii_control())
        {
            return Err(AgentDomainError::InvalidIdempotencyKey);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IdempotencyKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentDigest(String);

impl ContentDigest {
    pub const HEX_LENGTH: usize = 64;

    pub fn parse(value: impl Into<String>) -> Result<Self, AgentDomainError> {
        let value = value.into();
        if value.len() != Self::HEX_LENGTH
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(AgentDomainError::InvalidContentDigest);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut encoded = String::with_capacity(Self::HEX_LENGTH);
        for byte in digest {
            use fmt::Write as _;
            let _ = write!(&mut encoded, "{byte:02x}");
        }
        Self(encoded)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AggregateVersion(u64);

impl AggregateVersion {
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Result<Self, AgentDomainError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(AgentDomainError::VersionOverflow)
    }
}

impl Default for AggregateVersion {
    fn default() -> Self {
        Self::INITIAL
    }
}
