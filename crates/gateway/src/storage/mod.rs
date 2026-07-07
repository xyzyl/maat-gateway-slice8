//! Receipt storage abstraction.
//!
//! `ReceiptStore` is the trait that both backends implement.
//!
//! 0.4.0 also hosts the Redis-backed durable replay store and
//! distributed rate counter under this module tree â€” they're storage
//! adapters, even if not receipt-storage proper.

use async_trait::async_trait;
use maat::Receipt;
use serde::{Deserialize, Serialize};

pub mod filesystem;
pub mod postgres;
pub mod rate_redis;
pub mod replay_redis;

pub use filesystem::FilesystemStore;
pub use postgres::PostgresStore;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("storage backend error: {0}")]
    Backend(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("receipt not found")]
    NotFound,
}

pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ReceiptQuery {
    pub tenant_id: Option<uuid::Uuid>,
    pub since: Option<u64>,
    pub until: Option<u64>,
    pub agent_pubkey: Option<Vec<u8>>,
    pub action_scope: Option<String>,
    pub outcome: Option<String>,
    pub delegation_id: Option<[u8; 32]>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[async_trait]
pub trait ReceiptStore: Send + Sync {
    async fn write(&self, tenant_id: uuid::Uuid, receipt: &Receipt) -> StoreResult<()>;
    async fn get(&self, tenant_id: uuid::Uuid, id: &[u8; 32]) -> StoreResult<Receipt>;
    async fn query(&self, query: &ReceiptQuery) -> StoreResult<Vec<Receipt>>;
    async fn count(&self, query: &ReceiptQuery) -> StoreResult<u64>;
    fn backend_name(&self) -> &'static str;
}

pub(crate) fn normalize_limit(requested: Option<u32>) -> u32 {
    requested.unwrap_or(100).clamp(1, 1000)
}

pub(crate) fn normalize_offset(requested: Option<u32>) -> u32 {
    requested.unwrap_or(0)
}
