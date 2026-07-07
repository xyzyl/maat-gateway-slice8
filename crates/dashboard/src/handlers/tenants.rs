//! Tenant info and API key management handlers.

use std::sync::Arc;

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{internal_error, not_found, bad_request};
use crate::middleware::{require_admin, AuthContext};
use crate::state::DashboardState;

#[derive(Debug, Serialize)]
pub struct TenantView {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

pub async fn get_current_tenant(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
) -> Response {
    match state.tenants.get(auth.tenant_id).await {
        Ok(t) => {
            let view = TenantView {
                id: t.id,
                slug: t.slug,
                name: t.name,
            };
            (StatusCode::OK, Json(view)).into_response()
        }
        Err(_) => not_found("tenant not found"),
    }
}

#[derive(Debug, Serialize)]
pub struct ApiKeyView {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_api_keys(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
) -> Response {
    match state.api_keys.list_for_tenant(auth.tenant_id).await {
        Ok(keys) => {
            let views: Vec<ApiKeyView> = keys
                .into_iter()
                .map(|k| ApiKeyView {
                    id: k.id,
                    name: k.name,
                    key_prefix: k.key_prefix,
                    created_at: k.created_at,
                    last_used_at: k.last_used_at,
                    revoked_at: k.revoked_at,
                })
                .collect();
            (StatusCode::OK, Json(views)).into_response()
        }
        Err(e) => internal_error(&e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub metadata: ApiKeyView,
    /// Returned once at creation time. Cannot be retrieved later.
    pub full_key: String,
}

pub async fn create_api_key(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Response {
    if let Err(r) = require_admin(&auth) {
        return r;
    }
    if req.name.trim().is_empty() {
        return bad_request("name must not be empty");
    }

    match state.api_keys.create(auth.tenant_id, &req.name).await {
        Ok(created) => {
            let resp = CreateApiKeyResponse {
                metadata: ApiKeyView {
                    id: created.metadata.id,
                    name: created.metadata.name,
                    key_prefix: created.metadata.key_prefix,
                    created_at: created.metadata.created_at,
                    last_used_at: created.metadata.last_used_at,
                    revoked_at: created.metadata.revoked_at,
                },
                full_key: created.full_key,
            };
            (StatusCode::CREATED, Json(resp)).into_response()
        }
        Err(e) => internal_error(&e.to_string()),
    }
}

pub async fn revoke_api_key(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&auth) {
        return r;
    }

    match state.api_keys.revoke(auth.tenant_id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(maat_config::ConfigError::NotFound) => not_found("api key not found"),
        Err(e) => internal_error(&e.to_string()),
    }
}
