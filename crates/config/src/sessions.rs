//! Sessions repository — server-side session tokens.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::errors::{ConfigError, ConfigResult};

pub const SESSION_LIFETIME_HOURS: i64 = 24;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct SessionRepo {
    pool: PgPool,
}

impl SessionRepo {
    pub fn new(pool: PgPool) -> Self {
        SessionRepo { pool }
    }

    pub async fn create(&self, user_id: Uuid) -> ConfigResult<Session> {
        let now = Utc::now();
        let expires_at = now + Duration::hours(SESSION_LIFETIME_HOURS);

        let row = sqlx::query(
            r#"
            INSERT INTO sessions (user_id, expires_at)
            VALUES ($1, $2)
            RETURNING id, user_id, created_at, expires_at, last_seen_at
            "#,
        )
        .bind(user_id)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?;

        row_to_session(&row)
    }

    pub async fn verify(&self, session_id: Uuid) -> ConfigResult<Session> {
        let row = sqlx::query(
            "SELECT id, user_id, created_at, expires_at, last_seen_at
             FROM sessions WHERE id = $1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?
        .ok_or(ConfigError::AuthFailed)?;

        let session = row_to_session(&row)?;

        if session.expires_at < Utc::now() {
            return Err(ConfigError::AuthFailed);
        }

        let new_expires = Utc::now() + Duration::hours(SESSION_LIFETIME_HOURS);
        let _ = sqlx::query(
            "UPDATE sessions SET last_seen_at = NOW(), expires_at = $2 WHERE id = $1",
        )
        .bind(session_id)
        .bind(new_expires)
        .execute(&self.pool)
        .await;

        Ok(session)
    }

    pub async fn delete(&self, session_id: Uuid) -> ConfigResult<()> {
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|e| ConfigError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn sweep_expired(&self) -> ConfigResult<u64> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at < NOW()")
            .execute(&self.pool)
            .await
            .map_err(|e| ConfigError::Database(e.to_string()))?;
        Ok(result.rows_affected())
    }
}

fn row_to_session(row: &sqlx::postgres::PgRow) -> ConfigResult<Session> {
    Ok(Session {
        id: row.try_get("id").map_err(|e| ConfigError::Database(e.to_string()))?,
        user_id: row
            .try_get("user_id")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        created_at: row
            .try_get("created_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        expires_at: row
            .try_get("expires_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        last_seen_at: row
            .try_get("last_seen_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
    })
}
