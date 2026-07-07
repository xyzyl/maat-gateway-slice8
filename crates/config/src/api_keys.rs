//! API keys repository — verification service credentials per tenant.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::errors::{ConfigError, ConfigResult};
use crate::hashing::{api_key_prefix, generate_api_key, verify_api_key};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyCreated {
    pub metadata: ApiKey,
    pub full_key: String,
}

#[derive(Clone)]
pub struct ApiKeyRepo {
    pool: PgPool,
}

impl ApiKeyRepo {
    pub fn new(pool: PgPool) -> Self {
        ApiKeyRepo { pool }
    }

    pub async fn create(&self, tenant_id: Uuid, name: &str) -> ConfigResult<ApiKeyCreated> {
        let (full_key, prefix, hash) = generate_api_key()?;

        let row = sqlx::query(
            r#"
            INSERT INTO api_keys (tenant_id, name, key_prefix, key_hash)
            VALUES ($1, $2, $3, $4)
            RETURNING id, tenant_id, name, key_prefix, created_at, last_used_at, revoked_at
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .bind(&prefix)
        .bind(&hash)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?;

        let metadata = row_to_api_key(&row)?;
        Ok(ApiKeyCreated { metadata, full_key })
    }

    pub async fn authenticate(&self, presented_key: &str) -> ConfigResult<ApiKey> {
        let prefix = api_key_prefix(presented_key).ok_or(ConfigError::AuthFailed)?;

        let rows = sqlx::query(
            r#"
            SELECT id, tenant_id, name, key_prefix, key_hash, created_at, last_used_at, revoked_at
            FROM api_keys
            WHERE key_prefix = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(&prefix)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?;

        for row in &rows {
            let hash: String = row
                .try_get("key_hash")
                .map_err(|e| ConfigError::Database(e.to_string()))?;
            if verify_api_key(presented_key, &hash) {
                let key = row_to_api_key(row)?;

                let pool = self.pool.clone();
                let id = key.id;
                tokio::spawn(async move {
                    let _ = sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1")
                        .bind(id)
                        .execute(&pool)
                        .await;
                });

                return Ok(key);
            }
        }

        Err(ConfigError::AuthFailed)
    }

    pub async fn list_for_tenant(&self, tenant_id: Uuid) -> ConfigResult<Vec<ApiKey>> {
        let rows = sqlx::query(
            "SELECT id, tenant_id, name, key_prefix, created_at, last_used_at, revoked_at
             FROM api_keys WHERE tenant_id = $1 ORDER BY created_at DESC",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?;

        rows.iter().map(row_to_api_key).collect()
    }

    pub async fn revoke(&self, tenant_id: Uuid, key_id: Uuid) -> ConfigResult<()> {
        let result = sqlx::query(
            "UPDATE api_keys SET revoked_at = NOW()
             WHERE tenant_id = $1 AND id = $2 AND revoked_at IS NULL",
        )
        .bind(tenant_id)
        .bind(key_id)
        .execute(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(ConfigError::NotFound);
        }
        Ok(())
    }
}

fn row_to_api_key(row: &sqlx::postgres::PgRow) -> ConfigResult<ApiKey> {
    Ok(ApiKey {
        id: row.try_get("id").map_err(|e| ConfigError::Database(e.to_string()))?,
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        name: row.try_get("name").map_err(|e| ConfigError::Database(e.to_string()))?,
        key_prefix: row
            .try_get("key_prefix")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        created_at: row
            .try_get("created_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        last_used_at: row
            .try_get("last_used_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        revoked_at: row
            .try_get("revoked_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
    })
}
