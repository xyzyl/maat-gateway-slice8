//! Dashboard configuration.

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,

    /// Postgres URL for the multi-tenancy + receipts databases (shared
    /// with the gateway).
    pub database_url: String,

    /// KMS base URL (e.g., http://127.0.0.1:9090).
    pub kms_url: String,

    /// Bearer token sent to the KMS on every request.
    pub kms_auth_token: String,

    /// Redis URL for revocation publish/subscribe.
    /// e.g., redis://127.0.0.1:6379/
    pub redis_url: String,

    /// If true, sets the Secure attribute on session cookies. Required in
    /// production behind HTTPS; disabled by default for local development.
    pub secure_cookies: bool,

    /// Comma-separated origins permitted to call this service via CORS.
    /// Empty / unset means CORS is disabled (same-origin or proxied
    /// deployments). Example: "https://dashboard.example.com"
    pub cors_allowed_origins: Vec<String>,
}

impl Config {
    /// Env vars:
    ///   MAAT_DASHBOARD_BIND       Default: 127.0.0.1:8081
    ///   DATABASE_URL              Postgres URL (required)
    ///   MAAT_KMS_URL              Default: http://127.0.0.1:9090
    ///   MAAT_REDIS_URL            Default: redis://127.0.0.1:6379/
    ///   MAAT_DASHBOARD_SECURE     "true" / "false". Default: false (dev)
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = std::env::var("MAAT_DASHBOARD_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8081".to_string());

        let database_url = std::env::var("DATABASE_URL")
            .or_else(|_| std::env::var("MAAT_CONFIG_DATABASE_URL"))
            .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?;

        let kms_url = std::env::var("MAAT_KMS_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string());

        let kms_auth_token = std::env::var("MAAT_KMS_AUTH_TOKEN").map_err(|_| {
            anyhow::anyhow!(
                "MAAT_KMS_AUTH_TOKEN is required (matches the value the KMS \
                 was started with)"
            )
        })?;

        let redis_url = std::env::var("MAAT_REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string());

        let secure_cookies = std::env::var("MAAT_DASHBOARD_SECURE")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        // CORS: comma-separated. Empty / unset means disabled.
        let cors_allowed_origins = std::env::var("MAAT_DASHBOARD_CORS_ORIGINS")
            .map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(Config {
            bind_addr,
            database_url,
            kms_url,
            kms_auth_token,
            redis_url,
            secure_cookies,
            cors_allowed_origins,
        })
    }
}
