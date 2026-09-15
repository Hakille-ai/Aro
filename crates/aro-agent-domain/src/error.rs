use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    Transient,
    Permanent,
    Permission,
    Validation,
    Quota,
    Provider,
    Model,
    Tool,
    Environment,
    UserRequired,
    Conflict,
    Fencing,
    Integrity,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AgentDomainError {
    #[error("invalid transition for {aggregate}: {from} -> {command}")]
    InvalidTransition {
        aggregate: &'static str,
        from: String,
        command: &'static str,
    },
    #[error("expected aggregate version {expected}, found {actual}")]
    VersionConflict { expected: u64, actual: u64 },
    #[error("a transition requiring a safe boundary was attempted before reaching one")]
    UnsafeBoundary,
    #[error("the action digest does not match the approval")]
    ApprovalDigestMismatch,
    #[error("the idempotency key is invalid")]
    InvalidIdempotencyKey,
    #[error("the content digest must be a lowercase SHA-256 digest")]
    InvalidContentDigest,
    #[error("aggregate version overflow")]
    VersionOverflow,
    #[error("invalid agent specification: {0}")]
    InvalidAgentSpec(String),
    #[error("canonical JSON serialization failed: {0}")]
    Canonicalization(String),
    #[error("domain invariant violated: {0}")]
    InvariantViolation(String),
}

impl AgentDomainError {
    #[must_use]
    pub const fn class(&self) -> ErrorClass {
        match self {
            Self::VersionConflict { .. } => ErrorClass::Conflict,
            Self::ApprovalDigestMismatch
            | Self::InvalidIdempotencyKey
            | Self::InvalidContentDigest
            | Self::InvalidAgentSpec(_)
            | Self::InvalidTransition { .. }
            | Self::UnsafeBoundary => ErrorClass::Validation,
            Self::Canonicalization(_) | Self::InvariantViolation(_) | Self::VersionOverflow => {
                ErrorClass::System
            }
        }
    }
}
