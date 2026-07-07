//! Gateway configuration (Slice 4).

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum StorageBackend {
    /// Dev-only. Not multi-tenant safe.
    Filesystem { dir: PathBuf },
    Postgres { database_url: String },
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub storage: StorageBackend,
    pub kms_url: String,
    /// Bearer token sent to the KMS on every request.
    pub kms_auth_token: String,
    pub config_database_url: String,
    /// Redis URL for revocation pub/sub.
    pub redis_url: String,
}

impl Config {
    /// Env vars:
    ///   MAAT_GATEWAY_BIND        Default: 0.0.0.0:8080
    ///   MAAT_GATEWAY_STORAGE     "filesystem" or "postgres". Default: postgres
    ///   MAAT_GATEWAY_RECEIPTS    Receipt dir (filesystem only)
    ///   DATABASE_URL             Postgres URL for the receipt store (required if postgres)
    ///   MAAT_CONFIG_DATABASE_URL Postgres URL for the config DB (defaults to DATABASE_URL)
    ///   MAAT_KMS_URL             Default: http://127.0.0.1:9090
    ///   MAAT_REDIS_URL           Default: redis://127.0.0.1:6379/
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = std::env::var("MAAT_GATEWAY_BIND")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string());

        let backend = std::env::var("MAAT_GATEWAY_STORAGE")
            .unwrap_or_else(|_| "postgres".to_string());

        let storage = match backend.as_str() {
            "filesystem" | "fs" => {
                let dir = std::env::var("MAAT_GATEWAY_RECEIPTS")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("./receipts"));
                StorageBackend::Filesystem { dir }
            }
            "postgres" | "pg" => {
                let database_url = std::env::var("DATABASE_URL")
                    .map_err(|_| anyhow::anyhow!("DATABASE_URL is not set"))?;
                StorageBackend::Postgres { database_url }
            }
            other => anyhow::bail!(
                "unknown MAAT_GATEWAY_STORAGE={:?}; expected 'filesystem' or 'postgres'",
                other
            ),
        };

        let config_database_url = std::env::var("MAAT_CONFIG_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .map_err(|_| {
                anyhow::anyhow!("MAAT_CONFIG_DATABASE_URL (or DATABASE_URL) is required")
            })?;

        let kms_url = std::env::var("MAAT_KMS_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string());

        let kms_auth_token = std::env::var("MAAT_KMS_AUTH_TOKEN").map_err(|_| {
            anyhow::anyhow!(
                "MAAT_KMS_AUTH_TOKEN is required (set it to the same value the KMS \
                 is configured with; generate one with `maat-kmsd generate-auth-token`)"
            )
        })?;

        let redis_url = std::env::var("MAAT_REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string());

        Ok(Config {
            bind_addr,
            storage,
            kms_url,
            kms_auth_token,
            config_database_url,
            redis_url,
        })
    }
}
