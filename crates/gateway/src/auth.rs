//! Authentication middleware for the verification service.
//!
//! Every `/v1/*` request except `/v1/health` must present a valid API key
//! in `Authorization: Bearer <key>`. The middleware looks up the tenant
//! and attaches a `TenantContext` to request extensions.

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use maat_config::{ApiKeyRepo, ConfigError};
use std::sync::Arc;
use uuid::Uuid;

use crate::state::GatewayState;

#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: Uuid,
    pub api_key_id: Uuid,
    pub kms_executor_key_id: String,
}

pub async fn require_api_key(
    State(state): State<Arc<GatewayState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let key = match extract_bearer(&req) {
        Some(k) => k,
        None => return unauthorized("missing or malformed Authorization header"),
    };

    let api_key_repo = ApiKeyRepo::new(state.config_pool.clone());
    let api_key = match api_key_repo.authenticate(&key).await {
        Ok(k) => k,
        Err(ConfigError::AuthFailed) => return unauthorized("invalid API key"),
        Err(e) => {
            tracing::warn!("auth failure: {}", e);
            return internal_error("authentication backend error");
        }
    };

    let tenant = match state.tenants.get(api_key.tenant_id).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("tenant lookup failed: {}", e);
            return internal_error("tenant lookup failed");
        }
    };

    let ctx = TenantContext {
        tenant_id: api_key.tenant_id,
        api_key_id: api_key.id,
        kms_executor_key_id: tenant.kms_executor_key_id,
    };

    req.extensions_mut().insert(ctx);
    next.run(req).await
}

fn extract_bearer(req: &Request) -> Option<String> {
    let header_value = req.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
    let key = header_value.strip_prefix("Bearer ")?.trim();
    if key.is_empty() {
        return None;
    }
    Some(key.to_string())
}

fn unauthorized(msg: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
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
