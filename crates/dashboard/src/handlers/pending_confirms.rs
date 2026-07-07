//! Pending human-confirm approvals (dashboard).
//!
//! - GET    /v1/pending-confirms                  → list pending entries
//! - POST   /v1/pending-confirms/{id}/approve     → approve a pending entry
//! - POST   /v1/pending-confirms/{id}/deny        → deny a pending entry

use std::sync::Arc;

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use maat_gateway::pending_confirms::PendingError;
use serde::Serialize;
use uuid::Uuid;

use crate::error::{bad_request, internal_error, not_found};
use crate::middleware::{require_admin, AuthContext};
use crate::state::DashboardState;

#[derive(Debug, Serialize)]
pub struct PendingView {
    pub id: Uuid,
    pub delegation_id_b64: String,
    pub anchor_nonce_b64: String,
    pub action_scope: String,
    pub action_description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub pending: Vec<PendingView>,
}

pub async fn list_pending(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
) -> Response {
    if let Err(r) = require_admin(&auth) {
        return r;
    }
    match state
        .pending_confirms
        .list_pending(auth.tenant_id, 100)
        .await
    {
        Ok(rows) => {
            let pending: Vec<_> = rows
                .into_iter()
                .map(|p| PendingView {
                    id: p.id,
                    delegation_id_b64: URL_SAFE_NO_PAD.encode(p.delegation_id),
                    anchor_nonce_b64: URL_SAFE_NO_PAD.encode(p.anchor_nonce),
                    action_scope: p.action_scope,
                    action_description: p.action_description,
                    created_at: p.created_at,
                })
                .collect();
            (StatusCode::OK, Json(ListResponse { pending })).into_response()
        }
        Err(e) => internal_error(&format!("list pending failed: {}", e)),
    }
}

pub async fn approve(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Response {
    if let Err(r) = require_admin(&auth) {
        return r;
    }
    let id = match Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return bad_request("invalid pending id"),
    };
    match state
        .pending_confirms
        .approve(id, auth.tenant_id, auth.user_id)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "approved" })),
        )
            .into_response(),
        Err(PendingError::BadTransition { .. }) => not_found("no pending entry with that id"),
        Err(e) => internal_error(&format!("approve failed: {}", e)),
    }
}

pub async fn deny(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> Response {
    if let Err(r) = require_admin(&auth) {
        return r;
    }
    let id = match Uuid::parse_str(&id_str) {
        Ok(u) => u,
        Err(_) => return bad_request("invalid pending id"),
    };
    match state
        .pending_confirms
        .deny(id, auth.tenant_id, auth.user_id)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "denied" })),
        )
            .into_response(),
        Err(PendingError::BadTransition { .. }) => not_found("no pending entry with that id"),
        Err(e) => internal_error(&format!("deny failed: {}", e)),
    }
}
