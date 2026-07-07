//! Receipt read handlers â€” tenant-scoped queries against the receipt store.
//!
//! The dashboard talks to the same Postgres database as the gateway's
//! receipt store. Rather than duplicate query logic, we use sqlx directly
//! to keep this crate's deps minimal â€” but every query has an explicit
//! tenant_id filter, just like the gateway's storage layer.

use std::sync::Arc;

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::error::{bad_request, internal_error, not_found};
use crate::middleware::AuthContext;
use crate::state::DashboardState;

#[derive(Debug, Default, Deserialize)]
pub struct ListReceiptParams {
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub scope: Option<String>,
    pub outcome: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ListReceiptsResponse {
    pub receipts: Vec<serde_json::Value>,
    pub total: u64,
}

pub async fn list_receipts(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<ListReceiptParams>,
) -> Response {
    if let Some(ref o) = params.outcome {
        if !matches!(o.as_str(), "Success" | "Failure" | "Partial") {
            return bad_request("outcome must be 'Success', 'Failure', or 'Partial'");
        }
    }

    let limit = params.limit.unwrap_or(100).clamp(1, 1000) as i64;
    let offset = params.offset.unwrap_or(0) as i64;

    let mut sql = String::from(
        "SELECT receipt_json FROM receipts WHERE tenant_id = $1",
    );
    let mut count_sql = String::from(
        "SELECT COUNT(*) FROM receipts WHERE tenant_id = $1",
    );
    let mut n: usize = 2;

    if params.since.is_some() {
        sql.push_str(&format!(" AND stored_at >= ${}", n));
        count_sql.push_str(&format!(" AND stored_at >= ${}", n));
        n += 1;
    }
    if params.until.is_some() {
        sql.push_str(&format!(" AND stored_at <= ${}", n));
        count_sql.push_str(&format!(" AND stored_at <= ${}", n));
        n += 1;
    }
    if params.scope.is_some() {
        sql.push_str(&format!(" AND action_scope = ${}", n));
        count_sql.push_str(&format!(" AND action_scope = ${}", n));
        n += 1;
    }
    if params.outcome.is_some() {
        sql.push_str(&format!(" AND outcome = ${}", n));
        count_sql.push_str(&format!(" AND outcome = ${}", n));
        n += 1;
    }
    sql.push_str(&format!(
        " ORDER BY stored_at DESC LIMIT ${} OFFSET ${}",
        n,
        n + 1
    ));

    // Apply bindings in the same order to both queries.
    let mut q = sqlx::query(&sql).bind(auth.tenant_id);
    let mut cq = sqlx::query_scalar::<_, i64>(&count_sql).bind(auth.tenant_id);
    if let Some(s) = params.since {
        let ts = chrono::DateTime::<chrono::Utc>::from_timestamp(s, 0)
            .unwrap_or_else(chrono::Utc::now);
        q = q.bind(ts);
        cq = cq.bind(ts);
    }
    if let Some(u) = params.until {
        let ts = chrono::DateTime::<chrono::Utc>::from_timestamp(u, 0)
            .unwrap_or_else(chrono::Utc::now);
        q = q.bind(ts);
        cq = cq.bind(ts);
    }
    if let Some(s) = params.scope.clone() {
        q = q.bind(s.clone());
        cq = cq.bind(s);
    }
    if let Some(o) = params.outcome.clone() {
        q = q.bind(o.clone());
        cq = cq.bind(o);
    }
    q = q.bind(limit).bind(offset);

    let rows = match q.fetch_all(&state.pool).await {
        Ok(r) => r,
        Err(e) => return internal_error(&format!("query failed: {}", e)),
    };
    let total = cq.fetch_one(&state.pool).await.unwrap_or(rows.len() as i64).max(0) as u64;

    let receipts: Vec<serde_json::Value> = rows
        .into_iter()
        .filter_map(|row| row.try_get::<serde_json::Value, _>("receipt_json").ok())
        .collect();

    (StatusCode::OK, Json(ListReceiptsResponse { receipts, total })).into_response()
}

pub async fn get_receipt(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_b64): Path<String>,
) -> Response {
    let bytes = match URL_SAFE_NO_PAD.decode(&id_b64) {
        Ok(b) if b.len() == 32 => b,
        Ok(_) => return bad_request("id must decode to 32 bytes"),
        Err(_) => return bad_request("id must be base64url-encoded"),
    };

    let row = match sqlx::query(
        "SELECT receipt_json FROM receipts WHERE tenant_id = $1 AND id = $2",
    )
    .bind(auth.tenant_id)
    .bind(&bytes)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return internal_error(&e.to_string()),
    };

    let Some(row) = row else {
        return not_found("receipt not found");
    };

    match row.try_get::<serde_json::Value, _>("receipt_json") {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => internal_error(&e.to_string()),
    }
}
