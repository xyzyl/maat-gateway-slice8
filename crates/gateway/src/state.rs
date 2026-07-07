//! Shared Gateway state. 0.4.0 production version.
//!
//! Major changes from 0.3.0:
//! - `Mutex<VerificationContext>` is GONE. State (replay nonces, rate
//!   counters) lives in Redis. Revocation state lives in `RevocationCache`
//!   (now sync-readable via parking_lot::RwLock).
//! - New fields: `replay_store`, `rate_store`, `custom_registry`,
//!   `pending_confirms`.

use std::collections::HashMap;
use std::sync::Arc;

use maat_config::{DelegationRepo, RevocationSubscriber, TenantRepo};
use sqlx::PgPool;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::info;

use crate::config::{Config, StorageBackend};
use crate::custom_constraints::{default_registry, CustomConstraintRegistry};
use crate::kms_client::KmsClient;
use crate::ledger::Ledger;
use crate::pending_confirms::PendingConfirmRepo;
use crate::revocation::{spawn_subscriber_task, warm_up_from_database, RevocationCache};
use crate::storage::rate_redis::RedisRateStore;
use crate::storage::replay_redis::RedisReplayStore;
use crate::storage::{FilesystemStore, PostgresStore, ReceiptStore};

pub struct GatewayState {
    pub config: Config,
    pub config_pool: PgPool,
    pub tenants: TenantRepo,
    pub store: Arc<dyn ReceiptStore>,

    pub revocations: RevocationCache,
    pub ledger: Arc<Ledger>,

    /// Redis-backed durable replay protection.
    pub replay_store: RedisReplayStore,
    /// Redis-backed distributed rate counter.
    pub rate_store: RedisRateStore,
    /// Gateway-side custom constraint evaluator registry.
    pub custom_registry: CustomConstraintRegistry,
    /// Postgres-backed pending human-confirm state machine.
    pub pending_confirms: PendingConfirmRepo,

    _subscriber: Option<JoinHandle<()>>,

    /// KMS client cache, keyed by tenant's executor key id.
    signers: RwLock<HashMap<String, Arc<KmsClient>>>,
    kms_url: String,
    kms_auth_token: String,
}

impl GatewayState {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        info!("Connecting to configuration database");
        let config_pool = maat_config::connect(&config.config_database_url)
            .await
            .map_err(|e| anyhow::anyhow!("config db connection failed: {}", e))?;

        let tenants = TenantRepo::new(config_pool.clone());

        let store: Arc<dyn ReceiptStore> = match &config.storage {
            StorageBackend::Filesystem { dir } => {
                tokio::fs::create_dir_all(dir).await?;
                Arc::new(FilesystemStore::new(dir.clone()))
            }
            StorageBackend::Postgres { database_url } => {
                let pg = PostgresStore::connect(database_url)
                    .await
                    .map_err(|e| anyhow::anyhow!("postgres connection failed: {}", e))?;
                Arc::new(pg)
            }
        };

        // ── Revocation pipeline ──
        let revocations = RevocationCache::new();
        let delegations_repo = DelegationRepo::new(config_pool.clone());
        warm_up_from_database(&revocations, &delegations_repo).await?;

        let subscriber = RevocationSubscriber::new(&config.redis_url)
            .map_err(|e| anyhow::anyhow!("revocation subscriber init failed: {}", e))?;
        subscriber
            .ping()
            .await
            .map_err(|e| anyhow::anyhow!("redis unreachable at {}: {}", config.redis_url, e))?;
        info!("Redis reachable at {}", config.redis_url);

        let subscriber_handle = spawn_subscriber_task(revocations.clone(), subscriber);

        // ── Ledger ──
        let ledger = Arc::new(
            Ledger::new(config_pool.clone(), &config.redis_url)
                .map_err(|e| anyhow::anyhow!("ledger init failed: {}", e))?,
        );

        // ── Distributed replay + rate stores (NEW in 0.4.0) ──
        let replay_store = RedisReplayStore::connect(&config.redis_url).await?;
        let rate_store = RedisRateStore::connect(&config.redis_url).await?;
        info!("Replay and rate stores ready (Redis)");

        // ── Custom-constraint registry ──
        let custom_registry = default_registry();
        info!("Custom-constraint registry ready");

        // ── Pending-confirms repo (Postgres) ──
        let pending_confirms = PendingConfirmRepo::new(config_pool.clone());
        info!("Pending-confirms repo ready");

        Ok(GatewayState {
            kms_url: config.kms_url.clone(),
            kms_auth_token: config.kms_auth_token.clone(),
            config,
            config_pool,
            tenants,
            store,
            revocations,
            ledger,
            replay_store,
            rate_store,
            custom_registry,
            pending_confirms,
            _subscriber: Some(subscriber_handle),
            signers: RwLock::new(HashMap::new()),
        })
    }

    /// Get (or lazily construct) a KMS signer for a tenant's executor key.
    /// `KmsClient` itself implements `maat::Signer`; no wrapper type needed.
    pub async fn signer_for(
        &self,
        kms_key_id: &str,
    ) -> anyhow::Result<Arc<KmsClient>> {
        {
            let cache = self.signers.read().await;
            if let Some(existing) = cache.get(kms_key_id) {
                return Ok(existing.clone());
            }
        }

        let client = KmsClient::new(
            self.kms_url.clone(),
            kms_key_id.to_string(),
            self.kms_auth_token.clone(),
        );
        client
            .warm_up()
            .await
            .map_err(|e| anyhow::anyhow!("KMS warm-up failed for {}: {}", kms_key_id, e))?;

        let arc_client = Arc::new(client);
        let mut cache = self.signers.write().await;
        let entry = cache
            .entry(kms_key_id.to_string())
            .or_insert_with(|| arc_client.clone());
        Ok(entry.clone())
    }
}
