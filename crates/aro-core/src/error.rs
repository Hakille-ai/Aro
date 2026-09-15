use thiserror::Error;

#[derive(Debug, Error)]
pub enum AroError {
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("local runtime is unavailable: {0}")]
    RuntimeUnavailable(String),
    #[error("memory store error: {0}")]
    Memory(String),
    #[error("voice runtime error: {0}")]
    Voice(String),
    #[error("security policy blocked the request: {0}")]
    Security(String),
    #[error("database transaction must be retried: {0}")]
    RetryableTransaction(String),
    #[error("unexpected error: {0}")]
    Unexpected(String),
}

pub type AroResult<T> = Result<T, AroError>;
