//! Session authentication middleware.
//!
//! Extracts the `maat_session` cookie, verifies the session against the
//! database, loads the associated user, and attaches an `AuthContext`
//! (user_id, tenant_id, role) to request extensions.
//!
//! Handlers pull `AuthContext` via axum's `Extension` extractor.
//! Tenant scoping happens by passing `auth.tenant_id` into every query.

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use maat_config::{ConfigError, UserRole};
use uuid::Uuid;

use crate::error::unauthorized;
use crate::state::DashboardState;

pub const SESSION_COOKIE: &str = "maat_session";

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub role: UserRole,
    pub session_id: Uuid,
}

pub async fn require_session(
    State(state): State<Arc<DashboardState>>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Response {
    let Some(cookie) = jar.get(SESSION_COOKIE) else {
        return unauthorized("not authenticated");
    };

    let session_id = match Uuid::parse_str(cookie.value()) {
        Ok(id) => id,
        Err(_) => return unauthorized("invalid session cookie"),
    };

    let session = match state.sessions.verify(session_id).await {
        Ok(s) => s,
        Err(ConfigError::AuthFailed) => return unauthorized("session expired or unknown"),
        Err(e) => {
            tracing::warn!("session verify failed: {}", e);
            return unauthorized("session check failed");
        }
    };

    let user = match state.users.get(session.user_id).await {
        Ok(u) => u,
        Err(_) => return unauthorized("user not found"),
    };

    let auth = AuthContext {
        user_id: user.id,
        tenant_id: user.tenant_id,
        role: user.role,
        session_id: session.id,
    };

    req.extensions_mut().insert(auth);
    next.run(req).await
}

/// Helper: confirm the auth context has admin role; return a 403 response if not.
// Err carries the ready-to-send Response; handlers `return r` on it directly,
// so boxing would just add noise at every call site.
#[allow(clippy::result_large_err)]
pub fn require_admin(auth: &AuthContext) -> Result<(), Response> {
    if auth.role == UserRole::Admin {
        Ok(())
    } else {
        Err(crate::error::forbidden("admin role required"))
    }
}
