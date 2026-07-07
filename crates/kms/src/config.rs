//! KMS configuration loaded from environment variables.

use std::path::PathBuf;

use crate::vault::{parse_master_key, VaultError};

#[derive(Debug, Clone)]
pub struct Config {
    /// Address to bind the KMS HTTP server to.
    pub bind_addr: String,

    /// Path to the encrypted vault file.
    pub vault_path: PathBuf,

    /// 32-byte master key used to encrypt/decrypt seeds in the vault.
    pub master_key: [u8; 32],

    /// Bearer token required by every endpoint except /health.
    /// Generate one with: `maat-kmsd generate-auth-token`.
    pub auth_token: String,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// Relevant vars:
    ///   MAAT_KMS_BIND        Bind address. Default: 127.0.0.1:9090
    ///                         (localhost-only by default — the KMS should
    ///                          never be exposed to the public internet)
    ///   MAAT_KMS_VAULT       Path to vault file. Default: ./kms-vault.json
    ///   MAAT_KMS_MASTER_KEY  Base64url-encoded 32-byte master key. REQUIRED.
    ///                         Generate one with: maat-kmsd generate-master-key
    ///   MAAT_KMS_AUTH_TOKEN  Bearer token for KMS API auth. REQUIRED.
    ///                         Generate one with: maat-kmsd generate-auth-token
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = std::env::var("MAAT_KMS_BIND")
            .unwrap_or_else(|_| "127.0.0.1:9090".to_string());

        let vault_path = std::env::var("MAAT_KMS_VAULT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./kms-vault.json"));

        let master_key_b64 = std::env::var("MAAT_KMS_MASTER_KEY").map_err(|_| {
            anyhow::anyhow!(
                "MAAT_KMS_MASTER_KEY is not set. Generate one with:\n  \
                 maat-kmsd generate-master-key"
            )
        })?;

        let master_key = parse_master_key(&master_key_b64)
            .map_err(|e: VaultError| anyhow::anyhow!("invalid master key: {}", e))?;

        let auth_token = std::env::var("MAAT_KMS_AUTH_TOKEN").map_err(|_| {
            anyhow::anyhow!(
                "MAAT_KMS_AUTH_TOKEN is not set. Generate one with:\n  \
                 maat-kmsd generate-auth-token"
            )
        })?;
        if auth_token.len() < 16 {
            anyhow::bail!("MAAT_KMS_AUTH_TOKEN must be at least 16 characters");
        }

        Ok(Config {
            bind_addr,
            vault_path,
            master_key,
            auth_token,
        })
    }
}
