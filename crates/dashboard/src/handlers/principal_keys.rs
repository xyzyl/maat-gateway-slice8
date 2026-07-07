//! Principal key handlers.
//!
//! Principal keys are tenant-owned signing keys used to create delegations.
//! The key material lives in the KMS; this service just stores metadata
//! (name, KMS key ID, public key) and exposes CRUD endpoints.

use std::sync::Arc;

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{bad_request, internal_error, not_found};
use crate::middleware::{require_admin, AuthContext};
use crate::state::DashboardState;

#[derive(Debug, Serialize)]
pub struct PrincipalKeyView {
    pub id: Uuid,
    pub name: String,
    pub kms_key_id: String,
    pub public_key_b64: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub retired_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_principal_keys(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
) -> Response {
    match state.principal_keys.list_for_tenant(auth.tenant_id).await {
        Ok(keys) => {
            let views: Vec<PrincipalKeyView> = keys
                .into_iter()
                .map(|k| PrincipalKeyView {
                    id: k.id,
                    name: k.name,
                    kms_key_id: k.kms_key_id,
                    public_key_b64: k.public_key_b64,
                    created_at: k.created_at,
                    retired_at: k.retired_at,
                })
                .collect();
            (StatusCode::OK, Json(views)).into_response()
        }
        Err(e) => internal_error(&e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreatePrincipalKeyRequest {
    pub name: String,
}

pub async fn create_principal_key(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreatePrincipalKeyRequest>,
) -> Response {
    if let Err(r) = require_admin(&auth) {
        return r;
    }
    if req.name.trim().is_empty() {
        return bad_request("name must not be empty");
    }

    // Ask the KMS to generate a fresh key.
    let kms_key = match state.kms.generate_key().await {
        Ok(k) => k,
        Err(e) => {
            return internal_error(&format!("kms generate_key failed: {}", e));
        }
    };

    // Persist the metadata.
    let record = match state
        .principal_keys
        .create(
            auth.tenant_id,
            &req.name,
            &kms_key.key_id,
            &kms_key.public_key_b64,
        )
        .await
    {
        Ok(r) => r,
        Err(e) => return internal_error(&e.to_string()),
    };

    let view = PrincipalKeyView {
        id: record.id,
        name: record.name,
        kms_key_id: record.kms_key_id,
        public_key_b64: record.public_key_b64,
        created_at: record.created_at,
        retired_at: record.retired_at,
    };

    (StatusCode::CREATED, Json(view)).into_response()
}

pub async fn retire_principal_key(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&auth) {
        return r;
    }

    match state.principal_keys.retire(auth.tenant_id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(maat_config::ConfigError::NotFound) => not_found("principal key not found"),
        Err(e) => internal_error(&e.to_string()),
    }
}
