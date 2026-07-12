use std::io;
use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("YAML frontmatter error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Kairos config is missing at {0}")]
    ConfigMissing(PathBuf),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("policy denied access: {0}")]
    PolicyDenied(String),
    #[error("brain not found: {0}")]
    BrainNotFound(String),
    #[error("brain is disabled: {0}")]
    BrainDisabled(String),
    #[error("no enabled brain is available")]
    NoEnabledBrain,
    #[error("local Ollama error: {0}")]
    Ollama(String),
    #[error("inference provider error: {0}")]
    Provider(String),
}
