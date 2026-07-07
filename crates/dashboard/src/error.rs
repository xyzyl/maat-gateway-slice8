//! Shared HTTP response helpers for dashboard handlers.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

pub fn json_error(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

pub fn bad_request(msg: &str) -> Response {
    json_error(StatusCode::BAD_REQUEST, msg)
}

pub fn unauthorized(msg: &str) -> Response {
    json_error(StatusCode::UNAUTHORIZED, msg)
}

pub fn forbidden(msg: &str) -> Response {
    json_error(StatusCode::FORBIDDEN, msg)
}

pub fn not_found(msg: &str) -> Response {
    json_error(StatusCode::NOT_FOUND, msg)
}

pub fn internal_error(msg: &str) -> Response {
    tracing::warn!("internal error: {}", msg);
    json_error(StatusCode::INTERNAL_SERVER_ERROR, msg)
}
