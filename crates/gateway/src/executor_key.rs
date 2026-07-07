//! `GET /v1/executor-key`
//!
//! Returns the public half of the gateway's executor signing key for
//! the authenticated tenant. Resources (stores, payment processors,
//! anything that honors Maat receipts) call this once at startup,
//! cache the result, and use it to verify receipt signatures locally
//! without further calls to the gateway.
//!
//! This is the first piece of "resource-facing" gateway surface — meant
//! to be consumed by the entities that act on receipts, not by the
//! agents that request them. Both audiences authenticate with the same
//! tenant API key today.

use std::sync::Arc;

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Serialize;

use crate::auth::TenantContext;
use crate::state::GatewayState;

#[derive(Debug, Serialize)]
pub struct ExecutorKeyView {
    pub tenant_id: uuid::Uuid,
    pub key_id: String,
    pub algorithm: String,
    pub public_key_b64: String,
}

pub async fn handler(
    State(state): State<Arc<GatewayState>>,
    Extension(tenant): Extension<TenantContext>,
) -> Response {
    // Use the same path the verify handler uses to obtain a KmsClient —
    // signer_for caches the client and warms up its public key once.
    let signer = match state.signer_for(&tenant.kms_executor_key_id).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "executor-key handler: signer fetch failed");
            return internal_error("executor key unavailable");
        }
    };

    // The KmsClient implements maat::Signer, which exposes public_key().
    use maat::Signer;
    let pk = signer.public_key();

    let view = ExecutorKeyView {
        tenant_id: tenant.tenant_id,
        key_id: tenant.kms_executor_key_id.clone(),
        algorithm: format!("{:?}", pk.algorithm).to_lowercase(),
        public_key_b64: URL_SAFE_NO_PAD.encode(&pk.key_data),
    };

    (StatusCode::OK, Json(view)).into_response()
}

fn internal_error(msg: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}
