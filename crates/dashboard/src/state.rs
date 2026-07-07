//! Shared dashboard state.
//!
//! Holds the database pool, repositories, and a KMS HTTP client. Each
//! authenticated request handler pulls the user's tenant_id from the
//! session middleware and uses it to scope every database query.

use std::sync::Arc;

use maat_config::{
    ApiKeyRepo, DelegationRepo, PrincipalKeyRepo, RevocationPublisher, SessionRepo, TenantRepo,
    UserRepo,
};
use sqlx::PgPool;
use tracing::info;

use maat_gateway::pending_confirms::PendingConfirmRepo;

use crate::config::Config;
use crate::kms_client::KmsHttpClient;

pub struct DashboardState {
    pub config: Config,
    pub pool: PgPool,

    pub tenants: TenantRepo,
    pub users: UserRepo,
    pub sessions: SessionRepo,
    pub api_keys: ApiKeyRepo,
    pub principal_keys: PrincipalKeyRepo,
    pub delegations: DelegationRepo,

    pub pending_confirms: PendingConfirmRepo,

    /// Stateless HTTP client for the KMS. Cheap to clone.
    pub kms: Arc<KmsHttpClient>,

    /// Publisher for cross-instance revocation events.
    pub revocations: RevocationPublisher,
}

impl DashboardState {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        
        info!("Connecting to configuration database");
        let pool = maat_config::connect(&config.database_url)
            .await
            .map_err(|e| anyhow::anyhow!("config db connection failed: {}", e))?;
        info!("Configuration database ready");

        let kms = Arc::new(KmsHttpClient::new(
            config.kms_url.clone(),
            config.kms_auth_token.clone(),
        ));
        kms.health_check()
            .await
            .map_err(|e| anyhow::anyhow!("KMS unreachable at {}: {}", config.kms_url, e))?;
        info!("KMS reachable at {}", config.kms_url);

        let revocations = RevocationPublisher::new(&config.redis_url)
            .map_err(|e| anyhow::anyhow!("redis publisher init failed: {}", e))?;
        revocations
            .ping()
            .await
            .map_err(|e| anyhow::anyhow!("redis unreachable at {}: {}", config.redis_url, e))?;
        info!("Redis reachable at {}", config.redis_url);

        Ok(DashboardState {
            tenants: TenantRepo::new(pool.clone()),
            users: UserRepo::new(pool.clone()),
            sessions: SessionRepo::new(pool.clone()),
            api_keys: ApiKeyRepo::new(pool.clone()),
            principal_keys: PrincipalKeyRepo::new(pool.clone()),
            delegations: DelegationRepo::new(pool.clone()),

            pending_confirms: PendingConfirmRepo::new(pool.clone()),
            
            pool,
            kms,
            revocations,
            config,
        })
    }
}
