//! Tenants repository.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::errors::{ConfigError, ConfigResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub kms_executor_key_id: String,
    pub settings: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct TenantRepo {
    pool: PgPool,
}

impl TenantRepo {
    pub fn new(pool: PgPool) -> Self {
        TenantRepo { pool }
    }

    pub async fn create(
        &self,
        slug: &str,
        name: &str,
        kms_executor_key_id: &str,
    ) -> ConfigResult<Tenant> {
        let row = sqlx::query(
            r#"
            INSERT INTO tenants (slug, name, kms_executor_key_id)
            VALUES ($1, $2, $3)
            RETURNING id, slug, name, kms_executor_key_id, settings_json, created_at
            "#,
        )
        .bind(slug)
        .bind(name)
        .bind(kms_executor_key_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                ConfigError::AlreadyExists(format!("tenant slug '{}'", slug))
            }
            other => ConfigError::Database(other.to_string()),
        })?;

        row_to_tenant(&row)
    }

    pub async fn get(&self, id: Uuid) -> ConfigResult<Tenant> {
        let row = sqlx::query(
            "SELECT id, slug, name, kms_executor_key_id, settings_json, created_at
             FROM tenants WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?
        .ok_or(ConfigError::NotFound)?;

        row_to_tenant(&row)
    }

    pub async fn get_by_slug(&self, slug: &str) -> ConfigResult<Tenant> {
        let row = sqlx::query(
            "SELECT id, slug, name, kms_executor_key_id, settings_json, created_at
             FROM tenants WHERE slug = $1",
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?
        .ok_or(ConfigError::NotFound)?;

        row_to_tenant(&row)
    }
}

fn row_to_tenant(row: &sqlx::postgres::PgRow) -> ConfigResult<Tenant> {
    Ok(Tenant {
        id: row.try_get("id").map_err(|e| ConfigError::Database(e.to_string()))?,
        slug: row.try_get("slug").map_err(|e| ConfigError::Database(e.to_string()))?,
        name: row.try_get("name").map_err(|e| ConfigError::Database(e.to_string()))?,
        kms_executor_key_id: row
            .try_get("kms_executor_key_id")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        settings: row
            .try_get("settings_json")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        created_at: row
            .try_get("created_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
    })
}
