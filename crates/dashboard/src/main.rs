//! Maat Dashboard daemon.

use std::net::SocketAddr;
use std::sync::Arc;

use tracing::info;

use maat_dashboard::{build_router, Config, DashboardState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "maat_dashboard=info".into()),
        )
        .init();

    let config = Config::from_env()?;
    info!("Starting Maat Dashboard");
    info!("  Bind address:    {}", config.bind_addr);
    info!("  KMS URL:         {}", config.kms_url);
    info!("  Database URL:    {}", redact_url(&config.database_url));
    info!("  Secure cookies:  {}", config.secure_cookies);
    if config.cors_allowed_origins.is_empty() {
        info!("  CORS:            disabled (same-origin or proxied deployment)");
    } else {
        info!("  CORS origins:    {:?}", config.cors_allowed_origins);
    }

    let state = Arc::new(DashboardState::new(config).await?);
    info!("Dashboard ready.");

    let app = build_router(state.clone());
    let addr: SocketAddr = state.config.bind_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Maat Dashboard listening on http://{}", addr);

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
