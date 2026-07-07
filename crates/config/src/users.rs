//! Users repository — dashboard user accounts with password authentication.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::errors::{ConfigError, ConfigResult};
use crate::hashing::{hash_password, verify_password};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Viewer,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Admin => "admin",
            UserRole::Viewer => "viewer",
        }
    }

    pub fn parse(s: &str) -> ConfigResult<Self> {
        match s {
            "admin" => Ok(UserRole::Admin),
            "viewer" => Ok(UserRole::Viewer),
            other => Err(ConfigError::InvalidInput(format!("role '{}'", other))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
    #[serde(skip)]
    pub(crate) password_hash: String,
}

#[derive(Clone)]
pub struct UserRepo {
    pool: PgPool,
}

impl UserRepo {
    pub fn new(pool: PgPool) -> Self {
        UserRepo { pool }
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        email: &str,
        password: &str,
        role: UserRole,
    ) -> ConfigResult<User> {
        let password_hash = hash_password(password)?;

        let row = sqlx::query(
            r#"
            INSERT INTO users (tenant_id, email, password_hash, role)
            VALUES ($1, $2, $3, $4)
            RETURNING id, tenant_id, email, password_hash, role, created_at, last_login_at
            "#,
        )
        .bind(tenant_id)
        .bind(email)
        .bind(&password_hash)
        .bind(role.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                ConfigError::AlreadyExists(format!("user '{}' in tenant", email))
            }
            other => ConfigError::Database(other.to_string()),
        })?;

        row_to_user(&row)
    }

    pub async fn authenticate(
        &self,
        tenant_id: Uuid,
        email: &str,
        password: &str,
    ) -> ConfigResult<User> {
        let row = sqlx::query(
            "SELECT id, tenant_id, email, password_hash, role, created_at, last_login_at
             FROM users WHERE tenant_id = $1 AND email = $2",
        )
        .bind(tenant_id)
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?
        .ok_or(ConfigError::AuthFailed)?;

        let user = row_to_user(&row)?;

        if !verify_password(password, &user.password_hash) {
            return Err(ConfigError::AuthFailed);
        }

        let _ = sqlx::query("UPDATE users SET last_login_at = NOW() WHERE id = $1")
            .bind(user.id)
            .execute(&self.pool)
            .await;

        Ok(user)
    }

    pub async fn get(&self, id: Uuid) -> ConfigResult<User> {
        let row = sqlx::query(
            "SELECT id, tenant_id, email, password_hash, role, created_at, last_login_at
             FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?
        .ok_or(ConfigError::NotFound)?;

        row_to_user(&row)
    }

    pub async fn list_for_tenant(&self, tenant_id: Uuid) -> ConfigResult<Vec<User>> {
        let rows = sqlx::query(
            "SELECT id, tenant_id, email, password_hash, role, created_at, last_login_at
             FROM users WHERE tenant_id = $1 ORDER BY created_at DESC",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?;

        rows.iter().map(row_to_user).collect()
    }

    pub async fn delete(&self, tenant_id: Uuid, user_id: Uuid) -> ConfigResult<()> {
        let result = sqlx::query("DELETE FROM users WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| ConfigError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(ConfigError::NotFound);
        }
        Ok(())
    }
}

fn row_to_user(row: &sqlx::postgres::PgRow) -> ConfigResult<User> {
    let role_str: String = row
        .try_get("role")
        .map_err(|e| ConfigError::Database(e.to_string()))?;

    Ok(User {
        id: row.try_get("id").map_err(|e| ConfigError::Database(e.to_string()))?,
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        email: row.try_get("email").map_err(|e| ConfigError::Database(e.to_string()))?,
        password_hash: row
            .try_get("password_hash")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        role: UserRole::parse(&role_str)?,
        created_at: row
            .try_get("created_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        last_login_at: row
            .try_get("last_login_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
    })
}
