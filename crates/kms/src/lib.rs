//! Maat Key Management Service — library surface.
//!
//! This crate exposes two things:
//!   1. A binary (`maat-kmsd`) that runs the KMS as an HTTP service.
//!   2. A library surface used by integration tests and by gateway code
//!      that needs to spin up a KMS in-process for testing.

pub mod api;
pub mod auth;
pub mod config;
pub mod vault;

pub use api::{
    generate_key_handler, get_key_handler, health_handler, sign_handler, AppState,
    GenerateKeyResponse, SignRequest, SignResponse,
};
pub use auth::require_auth_token;
pub use config::Config;
pub use vault::{generate_master_key_b64, parse_master_key, PublicKeyView, Vault, VaultError};

use std::sync::Arc;

/// Build the axum router for the KMS. All routes except /health
/// require the bearer token in `state.auth_token`.
pub fn router(state: Arc<AppState>) -> axum::Router {
    use axum::{middleware, routing};

    let protected = axum::Router::new()
        .route("/kms/v1/keys", routing::post(generate_key_handler))
        .route("/kms/v1/keys/:id", routing::get(get_key_handler))
        .route("/kms/v1/keys/:id/sign", routing::post(sign_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_auth_token,
        ));

    let public = axum::Router::new().route("/kms/v1/health", routing::get(health_handler));

    protected.merge(public).with_state(state)
}

/// Generate a random 32-byte URL-safe-base64 auth token. Returned as a
/// String so callers can `println!` it directly.
pub fn generate_auth_token_b64() -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
