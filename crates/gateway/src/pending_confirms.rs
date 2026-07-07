//! Pending human-confirmation state machine.
//!
//! When a verify request hits a delegation with `RequireHumanConfirm`,
//! the gateway records a pending entry in Postgres, returns 202
//! Accepted with the `pending_id`, and waits for a human to approve
//! via the dashboard. The agent retries with the same request bundle
//! once notified.
//!
//! Flow:
//!   1. Agent submits ActionRequest with RequireHumanConfirm constraint.
//!   2. Gateway runs verify with `PendingHumanConfirm` evaluator that
//!      checks for an existing approved entry keyed by
//!      (delegation_id, anchor_nonce). If approved, evaluator returns Ok.
//!      If pending or absent, evaluator returns the special "pending"
//!      error.
//!   3. On "pending" error, gateway:
//!      - Inserts/updates a pending row with status="pending" if absent.
//!      - Returns HTTP 202 with `pending_id` and a hint to retry after
//!        approval.
//!   4. Dashboard surfaces pending entries to admins, who approve or deny.
//!   5. Approval updates the row to status="approved" + approver_user_id.
//!   6. Agent re-submits the same request; this time the evaluator finds
//!      an approved row and lets verification proceed.
//!
//! Approval is keyed by `(tenant_id, delegation_id, anchor_nonce)` so
//! each anchor's invocation requires its own confirmation. Approvals
//! are single-use: once consumed by a successful verify, the row
//! transitions to status="consumed" so it can't gate later actions.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PendingConfirm {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub delegation_id: [u8; 32],
    pub anchor_nonce: [u8; 16],
    pub action_scope: String,
    pub action_description: Option<String>,
    pub status: ConfirmStatus,
    pub approver_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmStatus {
    Pending,
    Approved,
    Denied,
    Consumed,
}

impl ConfirmStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfirmStatus::Pending => "pending",
            ConfirmStatus::Approved => "approved",
            ConfirmStatus::Denied => "denied",
            ConfirmStatus::Consumed => "consumed",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "denied" => Some(Self::Denied),
            "consumed" => Some(Self::Consumed),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PendingError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("not found")]
    NotFound,
    #[error("cannot transition from {from} to {to}")]
    BadTransition { from: &'static str, to: &'static str },
}

impl From<sqlx::Error> for PendingError {
    fn from(e: sqlx::Error) -> Self {
        PendingError::Storage(e.to_string())
    }
}

#[derive(Clone)]
pub struct PendingConfirmRepo {
    pool: PgPool,
}

impl PendingConfirmRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Look up a pending entry by (tenant, delegation, anchor_nonce).
    pub async fn find(
        &self,
        tenant_id: Uuid,
        delegation_id: &[u8; 32],
        anchor_nonce: &[u8; 16],
    ) -> Result<Option<PendingConfirm>, PendingError> {
        let row = sqlx::query(
            "SELECT id, tenant_id, delegation_id, anchor_nonce, action_scope,
                    action_description, status, approver_user_id, created_at, decided_at
             FROM pending_confirms
             WHERE tenant_id = $1 AND delegation_id = $2 AND anchor_nonce = $3",
        )
        .bind(tenant_id)
        .bind(&delegation_id[..])
        .bind(&anchor_nonce[..])
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(row_to_pending))
    }

    /// Insert a new pending row (or no-op if one already exists for this key).
    pub async fn insert_pending(
        &self,
        tenant_id: Uuid,
        delegation_id: &[u8; 32],
        anchor_nonce: &[u8; 16],
        action_scope: &str,
        action_description: Option<&str>,
    ) -> Result<PendingConfirm, PendingError> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO pending_confirms
                (id, tenant_id, delegation_id, anchor_nonce, action_scope,
                 action_description, status, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, 'pending', $7)
             ON CONFLICT (tenant_id, delegation_id, anchor_nonce) DO NOTHING",
        )
        .bind(id)
        .bind(tenant_id)
        .bind(&delegation_id[..])
        .bind(&anchor_nonce[..])
        .bind(action_scope)
        .bind(action_description)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // Re-fetch to return the canonical row (the one we inserted, or
        // the one that already existed).
        self.find(tenant_id, delegation_id, anchor_nonce)
            .await?
            .ok_or(PendingError::NotFound)
    }

    /// Approve a pending entry. Only entries in `pending` state can be
    /// approved.
    pub async fn approve(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        approver_user_id: Uuid,
    ) -> Result<(), PendingError> {
        let res = sqlx::query(
            "UPDATE pending_confirms
             SET status = 'approved', approver_user_id = $1, decided_at = NOW()
             WHERE id = $2 AND tenant_id = $3 AND status = 'pending'",
        )
        .bind(approver_user_id)
        .bind(id)
        .bind(tenant_id)
        .execute(&self.pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(PendingError::BadTransition {
                from: "non-pending",
                to: "approved",
            });
        }
        Ok(())
    }

    /// Deny a pending entry.
    pub async fn deny(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        approver_user_id: Uuid,
    ) -> Result<(), PendingError> {
        let res = sqlx::query(
            "UPDATE pending_confirms
             SET status = 'denied', approver_user_id = $1, decided_at = NOW()
             WHERE id = $2 AND tenant_id = $3 AND status = 'pending'",
        )
        .bind(approver_user_id)
        .bind(id)
        .bind(tenant_id)
        .execute(&self.pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(PendingError::BadTransition {
                from: "non-pending",
                to: "denied",
            });
        }
        Ok(())
    }

    /// Mark a previously-approved entry as consumed (called atomically
    /// from the verify path after a successful retry).
    pub async fn consume(
        &self,
        tenant_id: Uuid,
        delegation_id: &[u8; 32],
        anchor_nonce: &[u8; 16],
    ) -> Result<bool, PendingError> {
        let res = sqlx::query(
            "UPDATE pending_confirms
             SET status = 'consumed'
             WHERE tenant_id = $1 AND delegation_id = $2 AND anchor_nonce = $3
               AND status = 'approved'",
        )
        .bind(tenant_id)
        .bind(&delegation_id[..])
        .bind(&anchor_nonce[..])
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() == 1)
    }

    /// List pending entries for a tenant (dashboard view).
    pub async fn list_pending(
        &self,
        tenant_id: Uuid,
        limit: i64,
    ) -> Result<Vec<PendingConfirm>, PendingError> {
        let rows = sqlx::query(
            "SELECT id, tenant_id, delegation_id, anchor_nonce, action_scope,
                    action_description, status, approver_user_id, created_at, decided_at
             FROM pending_confirms
             WHERE tenant_id = $1 AND status = 'pending'
             ORDER BY created_at DESC
             LIMIT $2",
        )
        .bind(tenant_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_pending).collect())
    }
}

fn row_to_pending(r: sqlx::postgres::PgRow) -> PendingConfirm {
    let did_vec: Vec<u8> = r.get("delegation_id");
    let mut did = [0u8; 32];
    did.copy_from_slice(&did_vec);
    let nonce_vec: Vec<u8> = r.get("anchor_nonce");
    let mut nonce = [0u8; 16];
    nonce.copy_from_slice(&nonce_vec);
    let status_str: String = r.get("status");
    PendingConfirm {
        id: r.get("id"),
        tenant_id: r.get("tenant_id"),
        delegation_id: did,
        anchor_nonce: nonce,
        action_scope: r.get("action_scope"),
        action_description: r.get("action_description"),
        status: ConfirmStatus::parse(&status_str).unwrap_or(ConfirmStatus::Pending),
        approver_user_id: r.get("approver_user_id"),
        created_at: r.get("created_at"),
        decided_at: r.get("decided_at"),
    }
}
