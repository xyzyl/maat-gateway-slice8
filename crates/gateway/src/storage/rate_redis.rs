//! Redis-backed distributed rate limiting.
//!
//! Sliding window via sorted set with atomic Lua check-and-record:
//!   1. ZREMRANGEBYSCORE drops entries older than now - period
//!   2. ZCARD counts remaining
//!   3. If count < max: ZADD + EXPIRE, return 1; else return 0.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use maat::verify::RateStore;
use maat::{MaatError, ObjectId, Result as MaatResult};
use redis::Script;
use std::sync::Arc;

const RATE_SCRIPT: &str = r#"
local key = KEYS[1]
local now = tonumber(ARGV[1])
local period = tonumber(ARGV[2])
local max = tonumber(ARGV[3])
local member = ARGV[4]

local window_start = now - period
redis.call('ZREMRANGEBYSCORE', key, '-inf', window_start)
local count = redis.call('ZCARD', key)
if count >= max then
    return 0
end
redis.call('ZADD', key, now, member)
redis.call('EXPIRE', key, period + 10)
return 1
"#;

#[derive(Clone)]
pub struct RedisRateStore {
    client: Arc<redis::Client>,
    script: Arc<Script>,
}

impl RedisRateStore {
    pub async fn connect(redis_url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| anyhow::anyhow!("redis client init: {}", e))?;
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| anyhow::anyhow!("redis connect: {}", e))?;
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("redis ping: {}", e))?;
        Ok(RedisRateStore {
            client: Arc::new(client),
            script: Arc::new(Script::new(RATE_SCRIPT)),
        })
    }

    pub async fn check_and_record_async(
        &self,
        delegation_id: &ObjectId,
        max: u32,
        period_seconds: u64,
        now: u64,
    ) -> Result<bool, redis::RedisError> {
        let key = format!("maat:rate:{}", URL_SAFE_NO_PAD.encode(delegation_id.0));
        let member = format!("{}:{}", now, rand::random::<u64>());

        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let result: i64 = self
            .script
            .key(&key)
            .arg(now)
            .arg(period_seconds)
            .arg(max as u64)
            .arg(&member)
            .invoke_async(&mut conn)
            .await?;
        Ok(result == 1)
    }
}

pub struct RateStoreAdapter<'a> {
    inner: &'a RedisRateStore,
}

impl<'a> RateStoreAdapter<'a> {
    pub fn new(inner: &'a RedisRateStore) -> Self {
        Self { inner }
    }
}

impl<'a> RateStore for RateStoreAdapter<'a> {
    fn check_and_record(
        &mut self,
        delegation_id: &ObjectId,
        max: u32,
        period_seconds: u64,
        now: u64,
    ) -> MaatResult<bool> {
        let inner = self.inner.clone();
        let did = delegation_id.clone();
        let result = tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(async move {
                inner
                    .check_and_record_async(&did, max, period_seconds, now)
                    .await
            })
        });
        result.map_err(|e| MaatError::Crypto(format!("rate store: {}", e)))
    }
}
