//! Redis-backed durable replay protection.
//!
//! Keying: `maat:replay:{nonce_pair_b64}` → presence indicates seen.
//! TTL bounded by `expiry - now` so entries self-evict.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use maat::verify::ReplayStore;
use maat::{MaatError, Result as MaatResult};
use std::sync::Arc;

/// Redis-backed replay store. Cloneable; holds `Arc<redis::Client>` and
/// opens a fresh multiplexed connection per call (cheap; the connection
/// is itself a shared handle to a single backing socket pool).
#[derive(Clone)]
pub struct RedisReplayStore {
    client: Arc<redis::Client>,
}

impl RedisReplayStore {
    pub async fn connect(redis_url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| anyhow::anyhow!("redis client init: {}", e))?;
        // Probe once to fail fast if Redis is unreachable.
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| anyhow::anyhow!("redis connect: {}", e))?;
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("redis ping: {}", e))?;
        Ok(RedisReplayStore {
            client: Arc::new(client),
        })
    }

    pub async fn check_and_record_async(
        &self,
        nonce_pair: ([u8; 16], [u8; 16]),
        expiry_unix: u64,
        now_unix: u64,
    ) -> Result<bool, redis::RedisError> {
        let mut key_bytes = [0u8; 32];
        key_bytes[..16].copy_from_slice(&nonce_pair.0);
        key_bytes[16..].copy_from_slice(&nonce_pair.1);
        let key = format!("maat:replay:{}", URL_SAFE_NO_PAD.encode(key_bytes));

        let ttl_secs = expiry_unix
            .saturating_sub(now_unix)
            .clamp(60, 86_400 * 30);

        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let result: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut conn)
            .await?;

        Ok(result.is_some())
    }
}

/// Synchronous adapter bridging async Redis into the sync `ReplayStore`
/// trait. The gateway runs on a multi-threaded tokio runtime where
/// `block_in_place` is safe; same precedent as `kms_client::KmsClient`
/// and `dashboard::kms_signer::KmsSigner`.
pub struct ReplayStoreAdapter<'a> {
    inner: &'a RedisReplayStore,
    now: u64,
}

impl<'a> ReplayStoreAdapter<'a> {
    pub fn new(inner: &'a RedisReplayStore, now: u64) -> Self {
        Self { inner, now }
    }
}

impl<'a> ReplayStore for ReplayStoreAdapter<'a> {
    fn check_and_record(
        &mut self,
        nonce_pair: ([u8; 16], [u8; 16]),
        expiry: u64,
    ) -> MaatResult<bool> {
        let inner = self.inner.clone();
        let now = self.now;
        let result = tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(async move {
                inner.check_and_record_async(nonce_pair, expiry, now).await
            })
        });
        result.map_err(|e| MaatError::Crypto(format!("replay store: {}", e)))
    }
}
