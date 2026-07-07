//! Maat Gateway — Slice 4 (multi-tenant verification service)

use std::net::SocketAddr;
use std::sync::Arc;

use tracing::info;

use maat_gateway::{build_router, Config, GatewayState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "maat_gateway=info".into()),
        )
        .init();

    let config = Config::from_env()?;
    info!("Starting Maat Gateway");
    info!("  Bind address:         {}", config.bind_addr);
    info!("  KMS URL:              {}", config.kms_url);
    info!("  Config database URL:  {}", redact_url(&config.config_database_url));

    let state = Arc::new(GatewayState::new(config).await?);
    info!("  Storage:              {}", state.store.backend_name());
    info!("  Gateway ready.");

    let app = build_router(state.clone());

    let addr: SocketAddr = state.config.bind_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Maat Gateway listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

fn redact_url(url: &str) -> String {
    match url.split_once("://") {
        Some((scheme, rest)) => match rest.split_once('@') {
            Some((_creds, host)) => format!("{}://<redacted>@{}", scheme, host),
            None => url.to_string(),
        },
        None => url.to_string(),
    }
}
