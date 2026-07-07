//! Principal keys repository — tenant signing keys for delegations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::errors::{ConfigError, ConfigResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrincipalKey {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub kms_key_id: String,
    pub public_key_b64: String,
    pub created_at: DateTime<Utc>,
    pub retired_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct PrincipalKeyRepo {
    pool: PgPool,
}

impl PrincipalKeyRepo {
    pub fn new(pool: PgPool) -> Self {
        PrincipalKeyRepo { pool }
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        name: &str,
        kms_key_id: &str,
        public_key_b64: &str,
    ) -> ConfigResult<PrincipalKey> {
        let row = sqlx::query(
            r#"
            INSERT INTO principal_keys (tenant_id, name, kms_key_id, public_key_b64)
            VALUES ($1, $2, $3, $4)
            RETURNING id, tenant_id, name, kms_key_id, public_key_b64, created_at, retired_at
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .bind(kms_key_id)
        .bind(public_key_b64)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?;

        row_to_principal_key(&row)
    }

    pub async fn get(&self, tenant_id: Uuid, id: Uuid) -> ConfigResult<PrincipalKey> {
        let row = sqlx::query(
            "SELECT id, tenant_id, name, kms_key_id, public_key_b64, created_at, retired_at
             FROM principal_keys WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?
        .ok_or(ConfigError::NotFound)?;

        row_to_principal_key(&row)
    }

    pub async fn list_for_tenant(&self, tenant_id: Uuid) -> ConfigResult<Vec<PrincipalKey>> {
        let rows = sqlx::query(
            "SELECT id, tenant_id, name, kms_key_id, public_key_b64, created_at, retired_at
             FROM principal_keys WHERE tenant_id = $1 ORDER BY created_at DESC",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?;

        rows.iter().map(row_to_principal_key).collect()
    }

    pub async fn retire(&self, tenant_id: Uuid, id: Uuid) -> ConfigResult<()> {
        let result = sqlx::query(
            "UPDATE principal_keys SET retired_at = NOW()
             WHERE tenant_id = $1 AND id = $2 AND retired_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(ConfigError::NotFound);
        }
        Ok(())
    }
}

fn row_to_principal_key(row: &sqlx::postgres::PgRow) -> ConfigResult<PrincipalKey> {
    Ok(PrincipalKey {
        id: row.try_get("id").map_err(|e| ConfigError::Database(e.to_string()))?,
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        name: row.try_get("name").map_err(|e| ConfigError::Database(e.to_string()))?,
        kms_key_id: row
            .try_get("kms_key_id")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        public_key_b64: row
            .try_get("public_key_b64")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        created_at: row
            .try_get("created_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        retired_at: row
            .try_get("retired_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
    })
}
