//! User management handlers (admin-only).

use std::sync::Arc;

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use maat_config::{ConfigError, UserRole};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{bad_request, forbidden, internal_error, not_found};
use crate::middleware::{require_admin, AuthContext};
use crate::state::DashboardState;

#[derive(Debug, Serialize)]
pub struct UserListItem {
    pub id: Uuid,
    pub email: String,
    pub role: UserRole,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_login_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_users(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
) -> Response {
    match state.users.list_for_tenant(auth.tenant_id).await {
        Ok(users) => {
            let items: Vec<UserListItem> = users
                .into_iter()
                .map(|u| UserListItem {
                    id: u.id,
                    email: u.email,
                    role: u.role,
                    created_at: u.created_at,
                    last_login_at: u.last_login_at,
                })
                .collect();
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => internal_error(&e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub role: UserRole,
}

pub async fn create_user(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateUserRequest>,
) -> Response {
    if let Err(r) = require_admin(&auth) {
        return r;
    }
    if req.email.trim().is_empty() {
        return bad_request("email must not be empty");
    }
    if req.password.len() < 8 {
        return bad_request("password must be at least 8 characters");
    }

    match state
        .users
        .create(auth.tenant_id, &req.email, &req.password, req.role)
        .await
    {
        Ok(user) => {
            let item = UserListItem {
                id: user.id,
                email: user.email,
                role: user.role,
                created_at: user.created_at,
                last_login_at: user.last_login_at,
            };
            (StatusCode::CREATED, Json(item)).into_response()
        }
        Err(ConfigError::AlreadyExists(_)) => {
            crate::error::json_error(StatusCode::CONFLICT, "user already exists")
        }
        Err(e) => internal_error(&e.to_string()),
    }
}

pub async fn delete_user(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&auth) {
        return r;
    }

    // Cannot delete yourself — easy footgun to prevent.
    if id == auth.user_id {
        return forbidden("cannot delete your own user");
    }

    match state.users.delete(auth.tenant_id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(ConfigError::NotFound) => not_found("user not found"),
        Err(e) => internal_error(&e.to_string()),
    }
}
