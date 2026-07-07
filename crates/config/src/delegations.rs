//! Delegations repository â€” tenant-scoped record of signed delegations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::errors::{ConfigError, ConfigResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationRecord {
    pub id: Vec<u8>,
    pub tenant_id: Uuid,
    pub principal_pubkey: Vec<u8>,
    pub agent_pubkey: Vec<u8>,
    pub parent_id: Option<Vec<u8>>,
    pub not_before: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
    pub scope_grants: Vec<String>,
    pub delegation_json: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revocation_reason: Option<String>,
}

#[derive(Clone)]
pub struct DelegationRepo {
    pool: PgPool,
}

pub struct CreateDelegation<'a> {
    pub id: &'a [u8],
    pub tenant_id: Uuid,
    pub principal_pubkey: &'a [u8],
    pub agent_pubkey: &'a [u8],
    pub parent_id: Option<&'a [u8]>,
    pub not_before: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
    pub scope_grants: &'a [String],
    pub delegation_json: &'a serde_json::Value,
}

impl DelegationRepo {
    pub fn new(pool: PgPool) -> Self {
        DelegationRepo { pool }
    }

    pub async fn insert(&self, d: CreateDelegation<'_>) -> ConfigResult<DelegationRecord> {
        let row = sqlx::query(
            r#"
            INSERT INTO delegations
                (id, tenant_id, principal_pubkey, agent_pubkey, parent_id,
                 not_before, not_after, scope_grants, delegation_json)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (id) DO NOTHING
            RETURNING id, tenant_id, principal_pubkey, agent_pubkey, parent_id,
                      not_before, not_after, scope_grants, delegation_json,
                      created_at, revoked_at, revocation_reason
            "#,
        )
        .bind(d.id)
        .bind(d.tenant_id)
        .bind(d.principal_pubkey)
        .bind(d.agent_pubkey)
        .bind(d.parent_id)
        .bind(d.not_before)
        .bind(d.not_after)
        .bind(d.scope_grants)
        .bind(d.delegation_json)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?;

        match row {
            Some(r) => row_to_record(&r),
            None => self.get(d.tenant_id, d.id).await,
        }
    }

    pub async fn get(&self, tenant_id: Uuid, id: &[u8]) -> ConfigResult<DelegationRecord> {
        let row = sqlx::query(
            "SELECT id, tenant_id, principal_pubkey, agent_pubkey, parent_id,
                    not_before, not_after, scope_grants, delegation_json,
                    created_at, revoked_at, revocation_reason
             FROM delegations WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?
        .ok_or(ConfigError::NotFound)?;

        row_to_record(&row)
    }

    pub async fn list_for_tenant(
        &self,
        tenant_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> ConfigResult<Vec<DelegationRecord>> {
        let rows = sqlx::query(
            "SELECT id, tenant_id, principal_pubkey, agent_pubkey, parent_id,
                    not_before, not_after, scope_grants, delegation_json,
                    created_at, revoked_at, revocation_reason
             FROM delegations WHERE tenant_id = $1
             ORDER BY created_at DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(tenant_id)
        .bind(limit.clamp(1, 1000) as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?;

        rows.iter().map(row_to_record).collect()
    }

    pub async fn mark_revoked(
        &self,
        tenant_id: Uuid,
        id: &[u8],
        reason: Option<&str>,
    ) -> ConfigResult<()> {
        let result = sqlx::query(
            "UPDATE delegations
             SET revoked_at = NOW(), revocation_reason = $3
             WHERE tenant_id = $1 AND id = $2 AND revoked_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .bind(reason)
        .execute(&self.pool)
        .await
        .map_err(|e| ConfigError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(ConfigError::NotFound);
        }
        Ok(())
    }

    /// List the IDs of every revoked delegation across all tenants.
    /// Called once at gateway startup to seed the in-memory revocation set.
    /// Returns raw 32-byte IDs.
    pub async fn list_revoked_ids(&self) -> ConfigResult<Vec<Vec<u8>>> {
        let rows = sqlx::query("SELECT id FROM delegations WHERE revoked_at IS NOT NULL")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ConfigError::Database(e.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            let id: Vec<u8> = row
                .try_get("id")
                .map_err(|e| ConfigError::Database(e.to_string()))?;
            out.push(id);
        }
        Ok(out)
    }
}

fn row_to_record(row: &sqlx::postgres::PgRow) -> ConfigResult<DelegationRecord> {
    Ok(DelegationRecord {
        id: row.try_get("id").map_err(|e| ConfigError::Database(e.to_string()))?,
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        principal_pubkey: row
            .try_get("principal_pubkey")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        agent_pubkey: row
            .try_get("agent_pubkey")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        parent_id: row
            .try_get("parent_id")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        not_before: row
            .try_get("not_before")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        not_after: row
            .try_get("not_after")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        scope_grants: row
            .try_get("scope_grants")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        delegation_json: row
            .try_get("delegation_json")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        created_at: row
            .try_get("created_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        revoked_at: row
            .try_get("revoked_at")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
        revocation_reason: row
            .try_get("revocation_reason")
            .map_err(|e| ConfigError::Database(e.to_string()))?,
    })
}
