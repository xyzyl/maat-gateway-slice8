//! KMS HTTP client used by the dashboard service.
//!
//! Unlike the gateway's KmsClient (which is bound to one specific key and
//! implements maat::Signer), the dashboard talks to the KMS for many
//! different keys: tenant principal keys for delegation creation,
//! and tenant principal keys for revocation signing. So this client is
//! simpler — just a thin async wrapper over the KMS's HTTP API.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum KmsError {
    #[error("KMS unreachable: {0}")]
    Unreachable(String),

    #[error("KMS returned status {0}")]
    Status(u16),

    #[error("response parse error: {0}")]
    Parse(String),

    #[error("encoding error: {0}")]
    Encoding(String),
}

pub type KmsResult<T> = Result<T, KmsError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedKey {
    pub key_id: String,
    pub algorithm: String,
    pub public_key_b64: String,
    #[serde(default)]
    pub created_at: u64,
}

#[derive(Debug, Serialize)]
struct SignBody<'a> {
    message_b64: &'a str,
}

#[derive(Debug, Deserialize)]
struct SignResponse {
    #[allow(dead_code)]
    key_id: String,
    #[allow(dead_code)]
    algorithm: String,
    signature_b64: String,
}

pub struct KmsHttpClient {
    base_url: String,
    auth_token: String,
    http: reqwest::Client,
}

impl KmsHttpClient {
    pub fn new(base_url: impl Into<String>, auth_token: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("reqwest client should build");
        KmsHttpClient {
            base_url: base_url.into(),
            auth_token: auth_token.into(),
            http,
        }
    }

    pub async fn health_check(&self) -> KmsResult<()> {
        // Health endpoint does not require auth — call it bare.
        let resp = self
            .http
            .get(format!("{}/kms/v1/health", self.base_url))
            .send()
            .await
            .map_err(|e| KmsError::Unreachable(e.to_string()))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(KmsError::Status(resp.status().as_u16()))
        }
    }

    /// POST /kms/v1/keys — generate a new key in the vault.
    pub async fn generate_key(&self) -> KmsResult<GeneratedKey> {
        let resp = self
            .http
            .post(format!("{}/kms/v1/keys", self.base_url))
            .bearer_auth(&self.auth_token)
            .send()
            .await
            .map_err(|e| KmsError::Unreachable(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(KmsError::Status(resp.status().as_u16()));
        }
        resp.json::<GeneratedKey>()
            .await
            .map_err(|e| KmsError::Parse(e.to_string()))
    }

    /// GET /kms/v1/keys/:id — fetch a key's public view.
    pub async fn get_key(&self, key_id: &str) -> KmsResult<GeneratedKey> {
        let resp = self
            .http
            .get(format!("{}/kms/v1/keys/{}", self.base_url, key_id))
            .bearer_auth(&self.auth_token)
            .send()
            .await
            .map_err(|e| KmsError::Unreachable(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(KmsError::Status(resp.status().as_u16()));
        }
        resp.json::<GeneratedKey>()
            .await
            .map_err(|e| KmsError::Parse(e.to_string()))
    }

    /// POST /kms/v1/keys/:id/sign — sign bytes with the key.
    /// Returns the raw signature bytes (already base64-decoded).
    pub async fn sign(&self, key_id: &str, message: &[u8]) -> KmsResult<Vec<u8>> {
        let body = SignBody {
            message_b64: &URL_SAFE_NO_PAD.encode(message),
        };
        let resp = self
            .http
            .post(format!("{}/kms/v1/keys/{}/sign", self.base_url, key_id))
            .bearer_auth(&self.auth_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| KmsError::Unreachable(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(KmsError::Status(resp.status().as_u16()));
        }
        let sr: SignResponse = resp
            .json()
            .await
            .map_err(|e| KmsError::Parse(e.to_string()))?;

        URL_SAFE_NO_PAD
            .decode(&sr.signature_b64)
            .map_err(|e| KmsError::Encoding(e.to_string()))
    }
}
