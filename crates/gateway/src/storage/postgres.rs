//! Postgres receipt store.
//!
//! Writes and queries receipts against the `receipts` table defined in
//! `migrations/0001_initial.sql`. Uses `sqlx` for compile-time-verified SQL.
//!
//! Indexes:
//!   - (stored_at desc)                        — time-range queries
//!   - (agent_pubkey, stored_at desc)          — agent activity
//!   - (delegation_id)                         — per-delegation audit
//!   - (action_scope, stored_at desc)          — scope-based filtering
//!   - (outcome, stored_at desc)               — rejection analysis
//!
//! Every query here should hit one of these indexes. If a future query
//! pattern does not, add an index — do not scan the table.

use async_trait::async_trait;
use maat::{Outcome, Receipt};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};

use super::{
    normalize_limit, normalize_offset, ReceiptQuery, ReceiptStore, StoreError, StoreResult,
};

pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    /// Connect to Postgres using the given URL. The pool is bounded to 10
    /// connections, which is plenty for the verification service's workload:
    /// every request results in at most one INSERT.
    pub async fn connect(database_url: &str) -> StoreResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|e| StoreError::Backend(format!("connect failed: {}", e)))?;

        // Sanity check the schema is present. We do NOT run migrations here —
        // the operator applies them explicitly with `sqlx migrate run` or psql.
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT FROM information_schema.tables
                WHERE table_schema = 'public' AND table_name = 'receipts'
            )",
        )
        .fetch_one(&pool)
        .await
        .map_err(|e| StoreError::Backend(format!("schema check failed: {}", e)))?;

        if !exists {
            return Err(StoreError::Backend(
                "receipts table not found — apply migrations first (sqlx migrate run)".into(),
            ));
        }

        Ok(PostgresStore { pool })
    }
}

/// Convert a Maat Outcome to the TEXT value stored in Postgres.
fn outcome_text(o: Outcome) -> &'static str {
    match o {
        Outcome::Success => "Success",
        Outcome::Failure => "Failure",
        Outcome::Partial => "Partial",
    }
}

#[async_trait]
impl ReceiptStore for PostgresStore {
    async fn write(&self, tenant_id: uuid::Uuid, receipt: &Receipt) -> StoreResult<()> {
        let receipt_json = serde_json::to_value(receipt)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;

        // Agent pubkey placeholder. Slice 5+ will pass the agent through
        // explicitly; for now, the executor's pubkey is stored as a stand-in
        // so the column is non-null.
        let agent_pubkey_placeholder = &receipt.executor.key_data;

        let chain_bytes: Vec<Vec<u8>> = receipt
            .delegation_chain
            .iter()
            .map(|id| id.0.to_vec())
            .collect();

        let executed_at = chrono::DateTime::<chrono::Utc>::from_timestamp(
            receipt.executed_at as i64,
            0,
        )
        .ok_or_else(|| StoreError::Backend("invalid executed_at timestamp".into()))?;

        sqlx::query(
            r#"
            INSERT INTO receipts (
                id, tenant_id, executor_pubkey, delegation_id, anchor_id,
                outcome, action_scope, executed_at, delegation_chain,
                agent_pubkey, receipt_json
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(&receipt.id.0[..])
        .bind(tenant_id)
        .bind(&receipt.executor.key_data)
        .bind(&receipt.delegation_id.0[..])
        .bind(&receipt.anchor_id.0[..])
        .bind(outcome_text(receipt.outcome))
        .bind(&receipt.action.scope_used)
        .bind(executed_at)
        .bind(&chain_bytes)
        .bind(agent_pubkey_placeholder)
        .bind(&receipt_json)
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Backend(format!("insert failed: {}", e)))?;

        Ok(())
    }

    async fn get(&self, tenant_id: uuid::Uuid, id: &[u8; 32]) -> StoreResult<Receipt> {
        let row = sqlx::query(
            "SELECT receipt_json FROM receipts WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(&id[..])
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StoreError::Backend(e.to_string()))?;

        let row = row.ok_or(StoreError::NotFound)?;
        let json: serde_json::Value = row
            .try_get("receipt_json")
            .map_err(|e| StoreError::Backend(e.to_string()))?;

        serde_json::from_value(json).map_err(|e| StoreError::Serialization(e.to_string()))
    }

    async fn query(&self, q: &ReceiptQuery) -> StoreResult<Vec<Receipt>> {
        // We build the query dynamically because the set of filters varies
        // per request. sqlx's query builder would work too, but direct SQL
        // with parameter binding keeps the index usage obvious.
        let (sql, bindings) = build_query_sql(q, false);
        let mut query = sqlx::query(&sql);
        for b in bindings {
            query = apply_binding(query, b);
        }

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StoreError::Backend(e.to_string()))?;

        let mut receipts = Vec::with_capacity(rows.len());
        for row in rows {
            let json: serde_json::Value = row
                .try_get("receipt_json")
                .map_err(|e| StoreError::Backend(e.to_string()))?;
            let receipt: Receipt = serde_json::from_value(json)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            receipts.push(receipt);
        }
        Ok(receipts)
    }

    async fn count(&self, q: &ReceiptQuery) -> StoreResult<u64> {
        let (sql, bindings) = build_query_sql(q, true);
        let mut query = sqlx::query(&sql);
        for b in bindings {
            query = apply_binding(query, b);
        }

        let row = query
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StoreError::Backend(e.to_string()))?;

        let count: i64 = row
            .try_get(0)
            .map_err(|e| StoreError::Backend(e.to_string()))?;

        Ok(count.max(0) as u64)
    }

    fn backend_name(&self) -> &'static str {
        "postgres"
    }
}

/// Intermediate binding representation so we can build dynamic SQL without
/// losing type information. This is the least-bad way to do dynamic filters
/// in sqlx without a query builder crate.
enum Binding {
    Bytes(Vec<u8>),
    Text(String),
    Ts(chrono::DateTime<chrono::Utc>),
    I64(i64),
    Uuid(uuid::Uuid),
}

fn apply_binding<'a>(
    q: sqlx::query::Query<'a, sqlx::Postgres, sqlx::postgres::PgArguments>,
    b: Binding,
) -> sqlx::query::Query<'a, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match b {
        Binding::Bytes(v) => q.bind(v),
        Binding::Text(s) => q.bind(s),
        Binding::Ts(t) => q.bind(t),
        Binding::I64(n) => q.bind(n),
        Binding::Uuid(u) => q.bind(u),
    }
}

/// Build a SELECT or COUNT query from a ReceiptQuery.
/// Returns (sql, bindings) where bindings are applied in order.
fn build_query_sql(q: &ReceiptQuery, count_only: bool) -> (String, Vec<Binding>) {
    let mut clauses: Vec<String> = Vec::new();
    let mut bindings: Vec<Binding> = Vec::new();
    let mut n = 1;

    // Tenant filter first so the (tenant_id, ...) composite indexes are used.
    if let Some(tid) = q.tenant_id {
        clauses.push(format!("tenant_id = ${}", n));
        bindings.push(Binding::Uuid(tid));
        n += 1;
    }

    if let Some(since) = q.since {
        let ts = chrono::DateTime::<chrono::Utc>::from_timestamp(since as i64, 0)
            .unwrap_or_else(chrono::Utc::now);
        clauses.push(format!("stored_at >= ${}", n));
        bindings.push(Binding::Ts(ts));
        n += 1;
    }
    if let Some(until) = q.until {
        let ts = chrono::DateTime::<chrono::Utc>::from_timestamp(until as i64, 0)
            .unwrap_or_else(chrono::Utc::now);
        clauses.push(format!("stored_at <= ${}", n));
        bindings.push(Binding::Ts(ts));
        n += 1;
    }
    if let Some(ref agent) = q.agent_pubkey {
        clauses.push(format!("agent_pubkey = ${}", n));
        bindings.push(Binding::Bytes(agent.clone()));
        n += 1;
    }
    if let Some(ref scope) = q.action_scope {
        clauses.push(format!("action_scope = ${}", n));
        bindings.push(Binding::Text(scope.clone()));
        n += 1;
    }
    if let Some(ref outcome) = q.outcome {
        clauses.push(format!("outcome = ${}", n));
        bindings.push(Binding::Text(outcome.clone()));
        n += 1;
    }
    if let Some(deleg_id) = q.delegation_id {
        clauses.push(format!("delegation_id = ${}", n));
        bindings.push(Binding::Bytes(deleg_id.to_vec()));
        n += 1;
    }

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };

    if count_only {
        let sql = format!("SELECT COUNT(*) FROM receipts{}", where_clause);
        (sql, bindings)
    } else {
        let limit = normalize_limit(q.limit) as i64;
        let offset = normalize_offset(q.offset) as i64;
        let sql = format!(
            "SELECT receipt_json FROM receipts{} ORDER BY stored_at DESC LIMIT ${} OFFSET ${}",
            where_clause,
            n,
            n + 1
        );
        bindings.push(Binding::I64(limit));
        bindings.push(Binding::I64(offset));
        (sql, bindings)
    }
}
