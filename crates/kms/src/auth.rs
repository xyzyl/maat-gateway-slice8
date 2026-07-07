//! Bearer-token authentication middleware for the KMS.
//!
//! Every endpoint except `/kms/v1/health` requires a valid bearer token
//! in the `Authorization` header. The token is compared against the
//! configured `MAAT_KMS_AUTH_TOKEN` in constant time to defend against
//! timing oracles.
//!
//! There's only one valid token per KMS instance — a single shared
//! secret across the gateway, dashboard, and bootstrap binary. Per-caller
//! tokens are a deployment-shape concern and a follow-up.

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use subtle::ConstantTimeEq;

use crate::api::AppState;

pub async fn require_auth_token(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let presented = match extract_bearer(&req) {
        Some(t) => t,
        None => return unauthorized("missing or malformed Authorization header"),
    };

    if !constant_time_token_eq(&presented, &state.auth_token) {
        return unauthorized("invalid auth token");
    }

    next.run(req).await
}

fn extract_bearer(req: &Request) -> Option<String> {
    let header_value = req.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = header_value.strip_prefix("Bearer ")?.trim();
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

fn constant_time_token_eq(presented: &str, expected: &str) -> bool {
    // ConstantTimeEq operates on byte slices of equal length. If the
    // lengths differ, they're trivially unequal — we still touch both
    // slices to avoid leaking length information through timing.
    let p = presented.as_bytes();
    let e = expected.as_bytes();
    if p.len() != e.len() {
        // Compare each against itself to keep the timing profile
        // independent of which length they are.
        let _ = p.ct_eq(p);
        let _ = e.ct_eq(e);
        return false;
    }
    p.ct_eq(e).into()
}

fn unauthorized(msg: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}
