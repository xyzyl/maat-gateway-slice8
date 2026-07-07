//! Internal error types for the Gateway.
//!
//! For Slice 1 we primarily rely on `anyhow::Error` for internal errors
//! and convert to HTTP responses in the handlers. This module is a
//! placeholder for more structured error types as the surface grows.

use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum GatewayError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("crypto error: {0}")]
    Crypto(String),
}
