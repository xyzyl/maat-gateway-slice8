//! KMS HTTP API.
//!
//! Three endpoints:
//!   POST /kms/v1/keys                    Generate a new key
//!   GET  /kms/v1/keys/:id                Fetch the public view of a key
//!   POST /kms/v1/keys/:id/sign           Sign bytes with the key
//!
//! Plus a health endpoint:
//!   GET  /kms/v1/health                  Liveness probe
//!
//! No authentication in Slice 3. The KMS is assumed to be network-isolated
//! and only reachable from the gateway over a private link (or localhost
//! during development). Authentication and mTLS land when multi-tenancy
//! arrives in Slice 4.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::vault::{PublicKeyView, Vault, VaultError};

/// Shared state held by all handlers.
pub struct AppState {
    pub vault: Vault,
    /// Required bearer token for all routes except /health.
    pub auth_token: String,
}

// ─── Responses ───

#[derive(Debug, Serialize)]
pub struct GenerateKeyResponse {
    pub key_id: String,
    pub algorithm: String,
    pub public_key_b64: String,
    pub created_at: u64,
}

impl From<PublicKeyView> for GenerateKeyResponse {
    fn from(v: PublicKeyView) -> Self {
        GenerateKeyResponse {
            key_id: v.key_id,
            algorithm: v.algorithm,
            public_key_b64: v.public_key_b64,
            created_at: v.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SignRequest {
    /// Bytes to sign, base64url-encoded.
    pub message_b64: String,
}

#[derive(Debug, Serialize)]
pub struct SignResponse {
    pub key_id: String,
    pub algorithm: String,
    pub signature_b64: String,
}

// ─── Handlers ───

/// POST /kms/v1/keys
pub async fn generate_key_handler(
    State(state): State<Arc<AppState>>,
) -> Response {
    match state.vault.generate_key().await {
        Ok(view) => {
            let resp: GenerateKeyResponse = view.into();
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => {
            warn!("key generation failed: {}", e);
            internal_error(&e.to_string())
        }
    }
}

/// GET /kms/v1/keys/:id
pub async fn get_key_handler(
    State(state): State<Arc<AppState>>,
    Path(key_id): Path<String>,
) -> Response {
    match state.vault.get_public(&key_id).await {
        Ok(view) => {
            let resp: GenerateKeyResponse = view.into();
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(VaultError::KeyNotFound(_)) => not_found("key not found"),
        Err(e) => {
            warn!("get_public failed: {}", e);
            internal_error(&e.to_string())
        }
    }
}

/// POST /kms/v1/keys/:id/sign
pub async fn sign_handler(
    State(state): State<Arc<AppState>>,
    Path(key_id): Path<String>,
    Json(req): Json<SignRequest>,
) -> Response {
    let message = match URL_SAFE_NO_PAD.decode(&req.message_b64) {
        Ok(v) => v,
        Err(_) => return bad_request("message_b64 must be base64url-encoded"),
    };

    match state.vault.sign(&key_id, &message).await {
        Ok(sig_bytes) => {
            info!(key_id = %key_id, msg_len = message.len(), "signed");
            let resp = SignResponse {
                key_id,
                algorithm: "ed25519".into(),
                signature_b64: URL_SAFE_NO_PAD.encode(&sig_bytes),
            };
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(VaultError::KeyNotFound(_)) => not_found("key not found"),
        Err(e) => {
            warn!(key_id = %key_id, error = %e, "sign failed");
            internal_error(&e.to_string())
        }
    }
}

/// GET /kms/v1/health
pub async fn health_handler() -> &'static str {
    "ok"
}

// ─── Response helpers ───

fn bad_request(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}

fn not_found(msg: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}

fn internal_error(msg: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}
