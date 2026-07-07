//! Maat Dashboard — library surface.
//!
//! Exposes the router builder, configuration, and types that integration
//! tests need to assemble their own server.

pub mod config;
pub mod error;
pub mod handlers;
pub mod kms_client;
pub mod kms_signer;
pub mod ledger;
pub mod middleware;
pub mod state;

pub use config::Config;
pub use middleware::{require_session, AuthContext, SESSION_COOKIE};
pub use state::DashboardState;

use std::sync::Arc;

use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post},
    Router,
};
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Build the full dashboard router. `/auth/login` is public; all other
/// routes require a valid session cookie.
pub fn build_router(state: Arc<DashboardState>) -> Router {
    let public = Router::new()
        .route("/dashboard/v1/auth/login", post(handlers::auth::login))
        .route("/dashboard/v1/health", get(|| async { "ok" }));

    let protected = Router::new()
        // Auth
        .route("/dashboard/v1/auth/logout", post(handlers::auth::logout))
        .route("/dashboard/v1/auth/me", get(handlers::auth::me))
        // Tenant
        .route(
            "/dashboard/v1/tenants/current",
            get(handlers::tenants::get_current_tenant),
        )
        .route(
            "/dashboard/v1/tenants/current/api-keys",
            get(handlers::tenants::list_api_keys).post(handlers::tenants::create_api_key),
        )
        .route(
            "/dashboard/v1/tenants/current/api-keys/:id",
            delete(handlers::tenants::revoke_api_key),
        )
        // Principal keys
        .route(
            "/dashboard/v1/principal-keys",
            get(handlers::principal_keys::list_principal_keys)
                .post(handlers::principal_keys::create_principal_key),
        )
        .route(
            "/dashboard/v1/principal-keys/:id/retire",
            post(handlers::principal_keys::retire_principal_key),
        )
        // Delegations
        .route(
            "/dashboard/v1/delegations",
            get(handlers::delegations::list_delegations)
                .post(handlers::delegations::create_delegation),
        )
        .route(
            "/dashboard/v1/delegations/:id_b64",
            get(handlers::delegations::get_delegation),
        )
        .route(
            "/dashboard/v1/delegations/:id_b64/revoke",
            post(handlers::delegations::revoke_delegation),
        )
        .route(
            "/dashboard/v1/delegations/:id_b64/ledger",
            get(handlers::delegations::get_ledger),
        )
        // Receipts
        .route(
            "/dashboard/v1/receipts",
            get(handlers::receipts::list_receipts),
        )
        .route(
            "/dashboard/v1/receipts/:id_b64",
            get(handlers::receipts::get_receipt),
        )
        // Users
        .route(
            "/dashboard/v1/users",
            get(handlers::users::list_users).post(handlers::users::create_user),
        )
        .route(
            "/dashboard/v1/users/:id",
            delete(handlers::users::delete_user),
        )
        .route(
            "/dashboard/v1/pending-confirms",
            get(handlers::pending_confirms::list_pending),
        )
        .route(
            "/dashboard/v1/pending-confirms/:id/approve",
            post(handlers::pending_confirms::approve),
        )
        .route(
            "/dashboard/v1/pending-confirms/:id/deny",
            post(handlers::pending_confirms::deny),
        )
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_session,
        ));

    let mut router = public.merge(protected).with_state(state.clone());

    // Apply CORS if any allowed origins were configured. Cookies require
    // a specific origin (not wildcard), so we whitelist exact strings and
    // set Allow-Credentials true.
    if !state.config.cors_allowed_origins.is_empty() {
        let origins: Vec<_> = state
            .config
            .cors_allowed_origins
            .iter()
            .filter_map(|s| s.parse::<axum::http::HeaderValue>().ok())
            .collect();

        let cors = CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_credentials(true)
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::DELETE,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers([axum::http::header::CONTENT_TYPE]);

        router = router.layer(cors);
    }

    router
}
