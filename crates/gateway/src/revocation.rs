//! In-memory revocation cache.
//!
//! 0.4.0: converted from `tokio::sync::RwLock` to `parking_lot::RwLock`.
//! Reads happen on every verify request and must be sync to integrate
//! with `verify_action_request_with`'s `is_revoked` closure.

use std::collections::HashSet;
use std::sync::Arc;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use maat_config::{DelegationRepo, RevocationEvent, RevocationSubscriber};
use parking_lot::RwLock;
use tracing::{info, warn};

#[derive(Clone, Default)]
pub struct RevocationCache {
    inner: Arc<RwLock<HashSet<[u8; 32]>>>,
}

impl RevocationCache {
    pub fn new() -> Self {
        RevocationCache::default()
    }

    /// Sync read for the verify hot path.
    pub fn is_revoked(&self, id: &[u8; 32]) -> bool {
        self.inner.read().contains(id)
    }

    /// Insert a delegation ID. Idempotent.
    pub fn insert(&self, id: [u8; 32]) {
        self.inner.write().insert(id);
    }

    /// Bulk-insert. Used by warm-up.
    pub fn extend(&self, ids: impl IntoIterator<Item = [u8; 32]>) {
        let mut guard = self.inner.write();
        guard.extend(ids);
    }

    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.read().is_empty()
    }
}

/// Load every currently-revoked delegation ID from the database into the
/// cache. Called once during gateway startup, before the HTTP server
/// begins accepting traffic.
pub async fn warm_up_from_database(
    cache: &RevocationCache,
    delegations: &DelegationRepo,
) -> anyhow::Result<usize> {
    let raw_ids = delegations
        .list_revoked_ids()
        .await
        .map_err(|e| anyhow::anyhow!("warm-up query failed: {}", e))?;

    let mut converted = Vec::with_capacity(raw_ids.len());
    for v in raw_ids {
        if v.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&v);
            converted.push(arr);
        }
    }
    let n = converted.len();
    cache.extend(converted);
    info!("Revocation warm-up loaded {} entries", n);
    Ok(n)
}

/// Spawn a background task that subscribes to revocation events and
/// inserts them into the cache as they arrive.
///
/// `RevocationSubscriber::run` drives a long-lived pub/sub loop with
/// exponential backoff and reconnects on its own; our handler is sync
/// and just dispatches a tiny async task per event to do the write.
pub fn spawn_subscriber_task(
    cache: RevocationCache,
    subscriber: RevocationSubscriber,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let cache_for_handler = cache.clone();
        let result = subscriber
            .run(move |event: RevocationEvent| {
                let cache = cache_for_handler.clone();
                tokio::spawn(async move {
                    apply_event(&cache, event);
                });
            })
            .await;
        if let Err(e) = result {
            warn!(error = %e, "revocation subscriber task ended");
        }
    })
}

fn apply_event(cache: &RevocationCache, event: RevocationEvent) {
    match URL_SAFE_NO_PAD.decode(&event.delegation_id_b64) {
        Ok(bytes) if bytes.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            cache.insert(arr);
            info!(
                id = %event.delegation_id_b64,
                "revocation event received and cached"
            );
        }
        Ok(_) => warn!(
            id = %event.delegation_id_b64,
            "revocation event delegation_id_b64 decoded to non-32-byte length; ignoring"
        ),
        Err(e) => warn!(
            id = %event.delegation_id_b64,
            error = %e,
            "revocation event delegation_id_b64 not valid base64url; ignoring"
        ),
    }
}
