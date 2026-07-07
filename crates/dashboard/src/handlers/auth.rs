//! Authentication endpoints.
//!
//!   POST /dashboard/v1/auth/login
//!   POST /dashboard/v1/auth/logout
//!   GET  /dashboard/v1/auth/me

use std::sync::Arc;

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use maat_config::{ConfigError, UserRole};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{internal_error, unauthorized};
use crate::middleware::{AuthContext, SESSION_COOKIE};
use crate::state::DashboardState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Tenant slug (e.g., "acme"). Email uniqueness is per-tenant, so login
    /// must specify which tenant the user belongs to.
    pub tenant: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserView {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub role: UserRole,
}

pub async fn login(
    State(state): State<Arc<DashboardState>>,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> Response {
    // Resolve tenant slug to ID first.
    let tenant = match state.tenants.get_by_slug(&req.tenant).await {
        Ok(t) => t,
        Err(ConfigError::NotFound) => return unauthorized("invalid credentials"),
        Err(e) => return internal_error(&e.to_string()),
    };

    let user = match state.users.authenticate(tenant.id, &req.email, &req.password).await {
        Ok(u) => u,
        Err(ConfigError::AuthFailed) => return unauthorized("invalid credentials"),
        Err(e) => return internal_error(&e.to_string()),
    };

    let session = match state.sessions.create(user.id).await {
        Ok(s) => s,
        Err(e) => return internal_error(&e.to_string()),
    };

    let mut cookie = Cookie::new(SESSION_COOKIE, session.id.to_string());
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Strict);
    cookie.set_path("/");
    cookie.set_secure(state.config.secure_cookies);

    let view = UserView {
        id: user.id,
        tenant_id: user.tenant_id,
        email: user.email.clone(),
        role: user.role,
    };

    (StatusCode::OK, jar.add(cookie), Json(view)).into_response()
}

pub async fn logout(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    jar: CookieJar,
) -> Response {
    if let Err(e) = state.sessions.delete(auth.session_id).await {
        tracing::warn!("session delete failed: {}", e);
    }

    let mut cookie = Cookie::new(SESSION_COOKIE, "");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Strict);
    cookie.set_path("/");
    cookie.set_secure(state.config.secure_cookies);
    cookie.make_removal();

    (StatusCode::NO_CONTENT, jar.add(cookie)).into_response()
}

pub async fn me(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
) -> Response {
    let user = match state.users.get(auth.user_id).await {
        Ok(u) => u,
        Err(_) => return unauthorized("user not found"),
    };
    let view = UserView {
        id: user.id,
        tenant_id: user.tenant_id,
        email: user.email,
        role: user.role,
    };
    (StatusCode::OK, Json(view)).into_response()
}
