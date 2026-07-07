//! Shared configuration database layer.
//!
//! Repositories for the multi-tenancy tables defined in
//! migrations/0002_multi_tenancy.sql. Both the gateway (for API key lookup)
//! and the dashboard (for everything else) depend on this crate.

pub mod api_keys;
pub mod delegations;
pub mod errors;
pub mod events;
pub mod hashing;
pub mod principal_keys;
pub mod sessions;
pub mod tenants;
pub mod users;

pub use api_keys::{ApiKey, ApiKeyRepo};
pub use delegations::{CreateDelegation, DelegationRecord, DelegationRepo};
pub use errors::{ConfigError, ConfigResult};
pub use events::{
    EventError, EventResult, RevocationEvent, RevocationPublisher, RevocationSubscriber,
    REVOCATION_CHANNEL,
};
pub use hashing::{generate_api_key, hash_api_key, hash_password, verify_api_key, verify_password};
pub use principal_keys::{PrincipalKey, PrincipalKeyRepo};
pub use sessions::{Session, SessionRepo};
pub use tenants::{Tenant, TenantRepo};
pub use users::{User, UserRepo, UserRole};

use sqlx::{postgres::PgPoolOptions, PgPool};

/// Connect to the configuration database and return a pool.
pub async fn connect(database_url: &str) -> ConfigResult<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await
        .map_err(|e| ConfigError::Database(format!("connect failed: {}", e)))?;

    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables
            WHERE table_schema = 'public' AND table_name = 'tenants'
        )",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ConfigError::Database(format!("schema check failed: {}", e)))?;

    if !exists {
        return Err(ConfigError::Database(
            "tenants table not found — apply migration 0002_multi_tenancy.sql first".into(),
        ));
    }

    Ok(pool)
}
