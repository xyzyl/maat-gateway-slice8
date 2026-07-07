//! GET /v1/receipts and GET /v1/receipts/:id handlers.
//!
//! Both pull the TenantContext from request extensions and scope every
//! query to the authenticated tenant.

use std::sync::Arc;

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use maat::Receipt;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::auth::TenantContext;
use crate::state::GatewayState;
use crate::storage::ReceiptQuery;

#[derive(Debug, Default, Deserialize)]
pub struct ListParams {
    pub since: Option<u64>,
    pub until: Option<u64>,
    pub agent: Option<String>,
    pub scope: Option<String>,
    pub outcome: Option<String>,
    pub delegation: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub receipts: Vec<Receipt>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

pub async fn list_handler(
    State(state): State<Arc<GatewayState>>,
    Extension(tenant): Extension<TenantContext>,
    Query(params): Query<ListParams>,
) -> Response {
    let agent_pubkey = match params
        .agent
        .as_ref()
        .map(|s| URL_SAFE_NO_PAD.decode(s))
        .transpose()
    {
        Ok(v) => v,
        Err(_) => return bad_request("agent must be base64url-encoded"),
    };

    let delegation_id: Option<[u8; 32]> = match params
        .delegation
        .as_ref()
        .map(|s| URL_SAFE_NO_PAD.decode(s))
        .transpose()
    {
        Ok(Some(v)) => match v.try_into() {
            Ok(arr) => Some(arr),
            Err(_) => return bad_request("delegation must be 32 bytes when decoded"),
        },
        Ok(None) => None,
        Err(_) => return bad_request("delegation must be base64url-encoded"),
    };

    if let Some(ref o) = params.outcome {
        if !matches!(o.as_str(), "Success" | "Failure" | "Partial") {
            return bad_request("outcome must be 'Success', 'Failure', or 'Partial'");
        }
    }

    let query = ReceiptQuery {
        tenant_id: Some(tenant.tenant_id),
        since: params.since,
        until: params.until,
        agent_pubkey,
        action_scope: params.scope,
        outcome: params.outcome,
        delegation_id,
        limit: params.limit,
        offset: params.offset,
    };

    let receipts = match state.store.query(&query).await {
        Ok(r) => r,
        Err(e) => {
            warn!("Query failed: {}", e);
            return internal_error("query failed");
        }
    };

    let total = state.store.count(&query).await.unwrap_or(receipts.len() as u64);

    let resp = ListResponse {
        receipts,
        total,
        limit: query.limit.unwrap_or(100).clamp(1, 1000),
        offset: query.offset.unwrap_or(0),
    };

    (StatusCode::OK, Json(resp)).into_response()
}

pub async fn get_handler(
    State(state): State<Arc<GatewayState>>,
    Extension(tenant): Extension<TenantContext>,
    Path(id_b64): Path<String>,
) -> Response {
    let bytes = match URL_SAFE_NO_PAD.decode(&id_b64) {
        Ok(v) => v,
        Err(_) => return bad_request("id must be base64url-encoded"),
    };
    let id_arr: [u8; 32] = match bytes.try_into() {
        Ok(a) => a,
        Err(_) => return bad_request("id must be 32 bytes when decoded"),
    };

    match state.store.get(tenant.tenant_id, &id_arr).await {
        Ok(receipt) => (StatusCode::OK, Json(receipt)).into_response(),
        Err(crate::storage::StoreError::NotFound) => {
            (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "not found" })))
                .into_response()
        }
        Err(e) => {
            warn!("get failed: {}", e);
            internal_error("get failed")
        }
    }
}

fn bad_request(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
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
