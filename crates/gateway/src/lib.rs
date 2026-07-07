//! Maat Gateway library surface (Slice 4).

pub mod auth;
pub mod config;
pub mod error;
pub mod executor_key;
pub mod kms_client;
pub mod ledger;
pub mod query;
pub mod revocation;
pub mod state;
pub mod storage;
pub mod verify;

pub mod custom_constraints;
pub mod failure_receipt;
pub mod human_confirm;
pub mod pending_confirms;


pub use auth::{require_api_key, TenantContext};
pub use config::{Config, StorageBackend};
pub use kms_client::KmsClient;
pub use ledger::{CumulativeCap, Ledger, LedgerError, LedgerSummary};
pub use query::{get_handler as receipt_get_handler, list_handler as receipt_list_handler};
pub use revocation::RevocationCache;
pub use state::GatewayState;
pub use verify::handler as verify_handler;

use std::sync::Arc;

/// Build the full router with auth middleware on tenant-scoped routes.
/// `/v1/health` is left unauthenticated.
pub fn build_router(state: Arc<GatewayState>) -> axum::Router {
    use axum::{middleware, routing::{get, post}, Router};

    let protected = Router::new()
        .route("/v1/verify", post(verify_handler))
        .route("/v1/receipts", get(receipt_list_handler))
        .route("/v1/receipts/:id", get(receipt_get_handler))
        .route("/v1/executor-key", get(executor_key::handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_key,
        ));

    let public = Router::new().route("/v1/health", get(|| async { "ok" }));

    protected.merge(public).with_state(state)
}
