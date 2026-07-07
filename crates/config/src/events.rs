//! Cross-instance event distribution.
//!
//! The dashboard publishes revocation events to a Redis channel. Every
//! running gateway subscribes to the same channel and updates its
//! in-memory revocation set when an event arrives.
//!
//! Redis is the right tool because we need fan-out to N subscribers with
//! sub-second latency, and we don't need durability of the event stream
//! itself — the database is the source of truth. Gateways do a one-time
//! load from the database at startup, then rely on pub/sub for the delta.
//!
//! ## Failure model
//! - Publisher unreachable: the dashboard returns 5xx on revoke (the
//!   database write succeeds, but we cannot promise propagation, so we
//!   fail loud). The operator can retry.
//! - Subscriber connection drops: the subscriber task logs and reconnects
//!   with exponential backoff. The gateway's revocation set continues to
//!   serve reads from whatever it had before the disconnect.
//! - Gateway restart: warm-up reload from the database catches anything
//!   missed during downtime.
//!
//! ## Wire format
//! Messages are JSON-encoded `RevocationEvent`s. Compact and easy to
//! observe with `redis-cli MONITOR` during development.

use std::time::Duration;

use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Default Redis pub/sub channel for revocation events.
pub const REVOCATION_CHANNEL: &str = "maat:revocations";

/// A revocation event. Sent by the dashboard, consumed by every gateway
/// subscribing to `REVOCATION_CHANNEL`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevocationEvent {
    /// Tenant the revocation applies to. Subscribers ignore events for
    /// tenants they don't have receipts for, but the gateway is global —
    /// it caches all revocations regardless of tenant.
    pub tenant_id: uuid::Uuid,

    /// The delegation's Maat ObjectId, base64url-encoded.
    pub delegation_id_b64: String,

    /// Optional reason supplied by the operator.
    pub reason: Option<String>,

    /// When the dashboard recorded the revocation. Clock-skew tolerant —
    /// the gateway uses this only for logging, not for any decision.
    pub revoked_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("redis error: {0}")]
    Redis(String),

    #[error("serialization error: {0}")]
    Serde(String),
}

pub type EventResult<T> = Result<T, EventError>;

impl From<redis::RedisError> for EventError {
    fn from(e: redis::RedisError) -> Self {
        EventError::Redis(e.to_string())
    }
}

/// Publishes events to a Redis channel.
///
/// Cheap to clone — the inner Redis client is reused.
#[derive(Clone)]
pub struct RevocationPublisher {
    client: redis::Client,
    channel: String,
}

impl RevocationPublisher {
    /// Connect (lazily) to Redis. Does not actually open a connection
    /// until `publish` is called.
    pub fn new(redis_url: &str) -> EventResult<Self> {
        let client = redis::Client::open(redis_url)?;
        Ok(RevocationPublisher {
            client,
            channel: REVOCATION_CHANNEL.to_string(),
        })
    }

    /// Sanity-check Redis is reachable. Used by service startup to
    /// fail-closed if Redis is misconfigured.
    pub async fn ping(&self) -> EventResult<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        Ok(())
    }

    /// Publish a revocation event. Returns the number of subscribers that
    /// received it, which is mostly useful for diagnostics.
    pub async fn publish(&self, event: &RevocationEvent) -> EventResult<u32> {
        let payload =
            serde_json::to_string(event).map_err(|e| EventError::Serde(e.to_string()))?;

        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let count: u32 = conn.publish(&self.channel, payload).await?;
        debug!(
            channel = %self.channel,
            subscribers = count,
            delegation = %event.delegation_id_b64,
            "published revocation"
        );
        Ok(count)
    }
}

/// A subscriber that receives events from a Redis channel and dispatches
/// them to a caller-supplied handler.
///
/// This type is consumed by spawning a task that runs `run` in a loop.
/// The handler closure is async-friendly: it returns immediately, so the
/// subscriber loop is never blocked on slow consumers.
pub struct RevocationSubscriber {
    client: redis::Client,
    channel: String,
}

impl RevocationSubscriber {
    pub fn new(redis_url: &str) -> EventResult<Self> {
        let client = redis::Client::open(redis_url)?;
        Ok(RevocationSubscriber {
            client,
            channel: REVOCATION_CHANNEL.to_string(),
        })
    }

    pub async fn ping(&self) -> EventResult<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        Ok(())
    }

    /// Run the subscription loop forever.
    ///
    /// `handler` is called once per event. If the handler panics or the
    /// connection drops, this function logs the failure, sleeps with
    /// exponential backoff (capped at 30s), and reconnects. It only
    /// returns if the handler asks it to (by returning `Err`).
    pub async fn run<F>(self, mut handler: F) -> EventResult<()>
    where
        F: FnMut(RevocationEvent),
    {
        use futures_util::StreamExt;

        let mut backoff_secs: u64 = 1;

        loop {
            // Each pass through the loop opens a fresh pub/sub connection.
            // If something fails, we drop it and start over.
            let conn = match self.client.get_async_pubsub().await {
                Ok(c) => c,
                Err(e) => {
                    warn!(
                        error = %e,
                        backoff_secs,
                        "redis pub/sub connect failed; will retry"
                    );
                    tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                    backoff_secs = (backoff_secs * 2).min(30);
                    continue;
                }
            };
            let mut conn = conn;

            if let Err(e) = conn.subscribe(&self.channel).await {
                warn!(error = %e, "redis SUBSCRIBE failed; will reconnect");
                tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                backoff_secs = (backoff_secs * 2).min(30);
                continue;
            }

            info!(channel = %self.channel, "revocation subscriber connected");
            backoff_secs = 1; // reset on successful subscribe

            // Pump messages until the connection ends.
            let mut stream = conn.on_message();
            while let Some(msg) = stream.next().await {
                let payload: String = match msg.get_payload() {
                    Ok(p) => p,
                    Err(e) => {
                        warn!(error = %e, "could not decode pub/sub payload");
                        continue;
                    }
                };
                match serde_json::from_str::<RevocationEvent>(&payload) {
                    Ok(ev) => handler(ev),
                    Err(e) => {
                        warn!(error = %e, payload = %payload, "malformed revocation event");
                    }
                }
            }

            // The stream ended. Either a clean disconnect or a network blip.
            warn!("revocation subscription ended; reconnecting");
            tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
            backoff_secs = (backoff_secs * 2).min(30);
        }
    }
}
