use thiserror::Error;

#[derive(Debug, Error)]
pub enum LabError {
    #[error("invalid stable identifier")]
    InvalidStableId,
    #[error("invalid SHA-256 digest")]
    InvalidSha256,
    #[error("invalid state transition")]
    InvalidTransition,
    #[error("unsupported schema or protocol version")]
    UnsupportedVersion,
    #[error("invalid or incomplete contract: {0}")]
    InvalidContract(&'static str),
    #[error("input exceeds the configured bound")]
    InputTooLarge,
    #[error("worker protocol violation: {0}")]
    WorkerProtocol(&'static str),
    #[error("worker run was cancelled")]
    WorkerCancelled,
    #[error("worker run exceeded its timeout")]
    WorkerTimedOut,
    #[error("registry writer is already active")]
    RegistryLocked,
    #[error("registry compare-and-swap failed")]
    StaleRegistry,
    #[error("requested record was not found")]
    NotFound,
    #[error("release is not eligible for activation")]
    NotEligible,
    #[error("record already exists")]
    AlreadyExists,
    #[error("unsafe worker executable or script")]
    UnsafeProcessPath,
    #[error("I/O operation failed")]
    Io(#[source] std::io::Error),
    #[error("invalid JSON")]
    Json(#[source] serde_json::Error),
}

impl From<std::io::Error> for LabError {
    fn from(source: std::io::Error) -> Self {
        Self::Io(source)
    }
}

impl From<serde_json::Error> for LabError {
    fn from(source: serde_json::Error) -> Self {
        Self::Json(source)
    }
}
