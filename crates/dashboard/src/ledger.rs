//! Dashboard-side access to the gateway's ledger tables.
//!
//! The dashboard does not need atomic check-and-increment (the gateway
//! owns that). It needs:
//!   - to write a cumulative cap row at delegation creation time
//!   - to read a ledger summary on demand for the UI
//!
//! Both are direct sqlx queries against the same tables the gateway uses.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CumulativeCap {
    pub currency: String,
    pub amount: u64,
    pub decimals: u8,
}

#[derive(Debug, Serialize)]
pub struct LedgerEntry {
    pub receipt_id_b64: String,
    pub currency: Option<String>,
    pub amount: Option<u64>,
    pub decimals: Option<u8>,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    pub reversed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct LedgerSummary {
    pub cap: Option<CumulativeCap>,
    pub recorded_total: u64,
    pub recent_entries: Vec<LedgerEntry>,
}

pub async fn set_cap(
    pool: &PgPool,
    tenant_id: Uuid,
    delegation_id: &[u8],
    cap: &CumulativeCap,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO delegation_cumulative_caps
         (tenant_id, delegation_id, currency, amount, decimals)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (tenant_id, delegation_id) DO UPDATE SET
            currency = EXCLUDED.currency,
            amount = EXCLUDED.amount,
            decimals = EXCLUDED.decimals",
    )
    .bind(tenant_id)
    .bind(delegation_id)
    .bind(&cap.currency)
    .bind(cap.amount as i64)
    .bind(cap.decimals as i16)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn summary(
    pool: &PgPool,
    tenant_id: Uuid,
    delegation_id: &[u8],
    recent_limit: i64,
) -> Result<LedgerSummary, sqlx::Error> {
    // Cap (optional)
    let cap_row = sqlx::query_as::<_, (String, i64, i16)>(
        "SELECT currency, amount, decimals FROM delegation_cumulative_caps
         WHERE tenant_id = $1 AND delegation_id = $2",
    )
    .bind(tenant_id)
    .bind(delegation_id)
    .fetch_optional(pool)
    .await?;
    let cap = cap_row.map(|(c, a, d)| CumulativeCap {
        currency: c,
        amount: a as u64,
        decimals: d as u8,
    });

    // Recorded total: sum non-reversed amounts. Note that this is the
    // POSTGRES view, which lags Redis on the hot path; for UI display
    // this is fine — the user is reading recent activity, not making
    // an enforcement decision.
    let recorded_total: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM ledger_entries
         WHERE tenant_id = $1 AND delegation_id = $2 AND reversed_at IS NULL",
    )
    .bind(tenant_id)
    .bind(delegation_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let entries = sqlx::query_as::<_, (
        Vec<u8>,
        Option<String>,
        Option<i64>,
        Option<i16>,
        chrono::DateTime<chrono::Utc>,
        Option<chrono::DateTime<chrono::Utc>>,
    )>(
        "SELECT receipt_id, currency, amount, decimals, recorded_at, reversed_at
         FROM ledger_entries
         WHERE tenant_id = $1 AND delegation_id = $2
         ORDER BY recorded_at DESC
         LIMIT $3",
    )
    .bind(tenant_id)
    .bind(delegation_id)
    .bind(recent_limit)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(rid, cur, amt, dec, rec, rev)| LedgerEntry {
        receipt_id_b64: URL_SAFE_NO_PAD.encode(&rid),
        currency: cur,
        amount: amt.map(|a| a as u64),
        decimals: dec.map(|d| d as u8),
        recorded_at: rec,
        reversed_at: rev,
    })
    .collect();

    Ok(LedgerSummary {
        cap,
        recorded_total: recorded_total as u64,
        recent_entries: entries,
    })
}
