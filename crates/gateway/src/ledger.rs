//! Cross-instance cumulative ledger for value-bearing delegations.
//!
//! The protocol's MaxValue enforces a per-action cap. The natural use case
//! ("$200 budget across all actions") needs cumulative state that's
//! shared across gateway instances. This module provides it.
//!
//! Architecture:
//! - Redis is the hot path. Atomic check-and-increment via a Lua script
//!   keeps two simultaneous verify requests from both succeeding when
//!   only one should.
//! - Postgres is the durable source of truth + query surface.
//! - The verify handler consults `check_and_reserve` BEFORE invoking the
//!   protocol, and `commit` AFTER a successful verify. If protocol
//!   verification fails between reserve and commit, `release` rolls back
//!   the Redis reservation.
//!
//! Keying: `maat:ledger:{tenant_id}:{delegation_id_b64}` holds the
//! current running total. The cap (if any) is fetched from Postgres on
//! ledger interaction; we don't cache it in Redis to avoid stale-cap
//! problems.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct CumulativeCap {
    pub currency: String,
    pub amount: u64,
    pub decimals: u8,
}

#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("redis error: {0}")]
    Redis(String),
    #[error("postgres error: {0}")]
    Postgres(String),
    #[error("currency mismatch: cap {cap}, claim {claim}")]
    CurrencyMismatch { cap: String, claim: String },
    #[error("decimals mismatch: cap {cap}, claim {claim}")]
    DecimalsMismatch { cap: u8, claim: u8 },
    #[error("cumulative cap exceeded: would be {would_be}, cap {cap}")]
    CapExceeded { would_be: u64, cap: u64 },
}

pub type LedgerResult<T> = Result<T, LedgerError>;

impl From<redis::RedisError> for LedgerError {
    fn from(e: redis::RedisError) -> Self {
        LedgerError::Redis(e.to_string())
    }
}
impl From<sqlx::Error> for LedgerError {
    fn from(e: sqlx::Error) -> Self {
        LedgerError::Postgres(e.to_string())
    }
}

/// Outcome of a check_and_reserve call.
#[derive(Debug)]
pub struct Reservation {
    pub redis_key: String,
    pub reserved_amount: u64,
    /// Total INCLUDING the reservation. Useful for receipt metadata.
    pub running_total_after: u64,
}

pub struct Ledger {
    pool: PgPool,
    redis: redis::Client,
}

impl Ledger {
    pub fn new(pool: PgPool, redis_url: &str) -> LedgerResult<Self> {
        let redis = redis::Client::open(redis_url)
            .map_err(|e| LedgerError::Redis(e.to_string()))?;
        Ok(Ledger { pool, redis })
    }

    /// Look up the cumulative cap for a delegation. None → no cap configured.
    pub async fn get_cap(
        &self,
        tenant_id: Uuid,
        delegation_id: &[u8],
    ) -> LedgerResult<Option<CumulativeCap>> {
        let row = sqlx::query_as::<_, (String, i64, i16)>(
            "SELECT currency, amount, decimals FROM delegation_cumulative_caps
             WHERE tenant_id = $1 AND delegation_id = $2",
        )
        .bind(tenant_id)
        .bind(delegation_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(currency, amount, decimals)| CumulativeCap {
            currency,
            amount: amount as u64,
            decimals: decimals as u8,
        }))
    }

    pub async fn set_cap(
        &self,
        tenant_id: Uuid,
        delegation_id: &[u8],
        cap: &CumulativeCap,
    ) -> LedgerResult<()> {
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
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Atomically check the cap and reserve `amount` against the ledger.
    ///
    /// Returns Ok(Some(Reservation)) when the reservation succeeded and
    /// the protocol verify can proceed. Returns Ok(None) when no cap is
    /// configured (protocol's per-action MaxValue still applies but
    /// cumulative state is irrelevant). Returns Err(CapExceeded) when
    /// the reservation would push past the cap.
    ///
    /// Currency/decimals must match the cap exactly — strict, no
    /// conversion.
    pub async fn check_and_reserve(
        &self,
        tenant_id: Uuid,
        delegation_id: &[u8],
        claim_currency: &str,
        claim_amount: u64,
        claim_decimals: u8,
    ) -> LedgerResult<Option<Reservation>> {
        let Some(cap) = self.get_cap(tenant_id, delegation_id).await? else {
            return Ok(None);
        };
        if cap.currency != claim_currency {
            return Err(LedgerError::CurrencyMismatch {
                cap: cap.currency,
                claim: claim_currency.to_string(),
            });
        }
        if cap.decimals != claim_decimals {
            return Err(LedgerError::DecimalsMismatch {
                cap: cap.decimals,
                claim: claim_decimals,
            });
        }

        let key = redis_key(tenant_id, delegation_id);
        let mut conn = self.redis.get_multiplexed_async_connection().await?;

        // Atomic check-and-increment via Lua: read current, compare to cap,
        // INCRBY iff within bounds. Returns -1 if exceeded, else new total.
        let script = redis::Script::new(
            r#"
            local current = tonumber(redis.call('GET', KEYS[1]) or '0')
            local incr = tonumber(ARGV[1])
            local cap = tonumber(ARGV[2])
            if current + incr > cap then
                return -1
            end
            return redis.call('INCRBY', KEYS[1], incr)
            "#,
        );
        let result: i64 = script
            .key(&key)
            .arg(claim_amount as i64)
            .arg(cap.amount as i64)
            .invoke_async(&mut conn)
            .await?;

        if result < 0 {
            // Need to compute "would_be" for a useful error. Read current.
            let current: Option<i64> = conn.get(&key).await?;
            let curr = current.unwrap_or(0) as u64;
            return Err(LedgerError::CapExceeded {
                would_be: curr + claim_amount,
                cap: cap.amount,
            });
        }

        Ok(Some(Reservation {
            redis_key: key,
            reserved_amount: claim_amount,
            running_total_after: result as u64,
        }))
    }

    /// Roll back a reservation when the protocol-level verify failed.
    /// Best-effort: a Redis hiccup here means the ledger temporarily
    /// over-counts until either a manual reset or eventually consistent
    /// reconciliation. Logs but does not propagate errors.
    pub async fn release(&self, reservation: &Reservation) {
        let mut conn = match self.redis.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "ledger release: redis unavailable; ledger may temporarily over-count");
                return;
            }
        };
        let _: Result<i64, _> = conn
            .decr(&reservation.redis_key, reservation.reserved_amount as i64)
            .await;
    }

    /// Commit a reservation by writing a durable ledger entry to Postgres.
    /// The Redis state is already correct; this just preserves the audit trail.
    pub async fn commit(
        &self,
        tenant_id: Uuid,
        delegation_id: &[u8],
        receipt_id: &[u8],
        currency: &str,
        amount: u64,
        decimals: u8,
    ) -> LedgerResult<()> {
        sqlx::query(
            "INSERT INTO ledger_entries
             (tenant_id, delegation_id, receipt_id, currency, amount, decimals)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(tenant_id)
        .bind(delegation_id)
        .bind(receipt_id)
        .bind(currency)
        .bind(amount as i64)
        .bind(decimals as i16)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Read-side: total spent + recent entries for a delegation.
    pub async fn summary(
        &self,
        tenant_id: Uuid,
        delegation_id: &[u8],
        recent_limit: i64,
    ) -> LedgerResult<LedgerSummary> {
        let cap = self.get_cap(tenant_id, delegation_id).await?;

        // Use Redis as authoritative running total (Postgres might lag if
        // a commit failed mid-flight, though in normal operation they agree).
        let key = redis_key(tenant_id, delegation_id);
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let running_total: i64 = conn.get(&key).await.unwrap_or(0);

        let entries = sqlx::query_as::<_, (Vec<u8>, Option<String>, Option<i64>, Option<i16>, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)>(
            "SELECT receipt_id, currency, amount, decimals, recorded_at, reversed_at
             FROM ledger_entries
             WHERE tenant_id = $1 AND delegation_id = $2
             ORDER BY recorded_at DESC
             LIMIT $3",
        )
        .bind(tenant_id)
        .bind(delegation_id)
        .bind(recent_limit)
        .fetch_all(&self.pool)
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
            running_total: running_total as u64,
            recent_entries: entries,
        })
    }
}

#[derive(Debug, serde::Serialize)]
pub struct LedgerSummary {
    pub cap: Option<CumulativeCap>,
    pub running_total: u64,
    pub recent_entries: Vec<LedgerEntry>,
}

#[derive(Debug, serde::Serialize)]
pub struct LedgerEntry {
    pub receipt_id_b64: String,
    pub currency: Option<String>,
    pub amount: Option<u64>,
    pub decimals: Option<u8>,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    pub reversed_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl serde::Serialize for CumulativeCap {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("CumulativeCap", 3)?;
        st.serialize_field("currency", &self.currency)?;
        st.serialize_field("amount", &self.amount)?;
        st.serialize_field("decimals", &self.decimals)?;
        st.end()
    }
}

fn redis_key(tenant_id: Uuid, delegation_id: &[u8]) -> String {
    format!(
        "maat:ledger:{}:{}",
        tenant_id,
        URL_SAFE_NO_PAD.encode(delegation_id)
    )
}
