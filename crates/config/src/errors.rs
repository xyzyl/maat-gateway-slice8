//! Error types for configuration database operations.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("database error: {0}")]
    Database(String),

    #[error("not found")]
    NotFound,

    #[error("already exists: {0}")]
    AlreadyExists(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("hashing error: {0}")]
    Hashing(String),

    #[error("authentication failed")]
    AuthFailed,
}

pub type ConfigResult<T> = Result<T, ConfigError>;
