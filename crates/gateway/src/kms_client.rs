//! HTTP client for the Maat KMS.
//!
//! `KmsClient` implements `maat::Signer`, so code that previously took a
//! `&Keypair` works unchanged — it just takes a `&KmsClient` instead.
//! This is the whole point of the Signer trait: it makes the signing
//! identity a plug.
//!
//! The client caches the public key after the first lookup so sign
//! operations don't do two network round trips per signature.

use std::sync::Arc;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use maat::{MaatError, PublicKey, Result as MaatResult, Signature, SignatureAlgorithm, Signer};
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;
use tracing::debug;

/// Shape returned by the KMS for key lookups / generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyView {
    key_id: String,
    algorithm: String,
    public_key_b64: String,
    #[serde(default)]
    created_at: u64,
}

/// Response shape for sign requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignResponse {
    key_id: String,
    algorithm: String,
    signature_b64: String,
}

/// Request body for sign calls.
#[derive(Debug, Serialize)]
struct SignBody<'a> {
    message_b64: &'a str,
}

/// A client bound to a specific KMS key. Construct one of these per key
/// the gateway wants to sign with. For Slice 3 the gateway uses one.
pub struct KmsClient {
    base_url: String,
    key_id: String,
    /// Bearer token sent on every KMS request.
    auth_token: String,
    http: reqwest::Client,
    /// The public key, fetched once on first access and cached.
    public_key: Arc<OnceCell<PublicKey>>,
}

impl KmsClient {
    /// Create a new client. `base_url` should be the KMS's root
    /// (e.g., "http://127.0.0.1:9090"). `key_id` identifies which key
    /// to use for signing. `auth_token` is the bearer token the KMS
    /// requires (set via MAAT_KMS_AUTH_TOKEN on the KMS side).
    pub fn new(
        base_url: impl Into<String>,
        key_id: impl Into<String>,
        auth_token: impl Into<String>,
    ) -> Self {
        KmsClient {
            base_url: base_url.into(),
            key_id: key_id.into(),
            auth_token: auth_token.into(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("reqwest client should build"),
            public_key: Arc::new(OnceCell::new()),
        }
    }

    /// Fetch the public key from the KMS and cache it.
    async fn fetch_public_key(&self) -> MaatResult<PublicKey> {
        let url = format!("{}/kms/v1/keys/{}", self.base_url, self.key_id);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.auth_token)
            .send()
            .await
            .map_err(|e| MaatError::Crypto(format!("KMS unreachable: {}", e)))?;

        if !resp.status().is_success() {
            return Err(MaatError::Crypto(format!(
                "KMS returned {} for key lookup",
                resp.status()
            )));
        }

        let view: KeyView = resp
            .json()
            .await
            .map_err(|e| MaatError::Crypto(format!("KMS response parse: {}", e)))?;

        let key_bytes = URL_SAFE_NO_PAD
            .decode(&view.public_key_b64)
            .map_err(|e| MaatError::Crypto(format!("bad public key b64: {}", e)))?;

        let algorithm = match view.algorithm.as_str() {
            "ed25519" => SignatureAlgorithm::Ed25519,
            other => return Err(MaatError::Crypto(format!(
                "unsupported algorithm: {}", other
            ))),
        };

        Ok(PublicKey {
            algorithm,
            key_data: key_bytes,
        })
    }

    /// Generate a new key on the KMS and return a client bound to it.
    /// Convenience for gateway startup when MAAT_KMS_KEY_ID is not set.
    pub async fn generate_key(base_url: &str, auth_token: &str) -> MaatResult<Self> {
        let http = reqwest::Client::new();
        let url = format!("{}/kms/v1/keys", base_url);
        let resp = http
            .post(&url)
            .bearer_auth(auth_token)
            .send()
            .await
            .map_err(|e| MaatError::Crypto(format!("KMS unreachable: {}", e)))?;

        if !resp.status().is_success() {
            return Err(MaatError::Crypto(format!(
                "KMS returned {} for key generation",
                resp.status()
            )));
        }

        let view: KeyView = resp
            .json()
            .await
            .map_err(|e| MaatError::Crypto(format!("KMS response parse: {}", e)))?;

        debug!(key_id = %view.key_id, "generated KMS key");
        Ok(KmsClient::new(base_url.to_string(), view.key_id, auth_token.to_string()))
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Ensure the public key is cached. Called at startup so the gateway
    /// fails fast if the KMS is misconfigured.
    pub async fn warm_up(&self) -> MaatResult<()> {
        self.get_public_key_cached().await?;
        Ok(())
    }

    async fn get_public_key_cached(&self) -> MaatResult<&PublicKey> {
        self.public_key
            .get_or_try_init(|| async { self.fetch_public_key().await })
            .await
    }
}

/// `Signer` is the Maat trait that `Keypair` already implements. By
/// implementing it for `KmsClient`, any Maat code that previously accepted
/// `&Keypair` now works unchanged with `&KmsClient`. This is why the
/// refactor from Keypair → KmsClient is small.
impl Signer for KmsClient {
    fn public_key(&self) -> PublicKey {
        // The trait returns synchronously, so we need the cache to be
        // populated before this is first called. `warm_up()` handles that.
        // If the cache is empty, we panic with a clear message rather than
        // returning a wrong key — this is a programmer error, not a
        // runtime condition we handle silently.
        match self.public_key.get() {
            Some(pk) => pk.clone(),
            None => panic!(
                "KmsClient::public_key() called before warm_up(); \
                 call warm_up() during gateway startup"
            ),
        }
    }

    fn sign(&self, message: &[u8]) -> MaatResult<Signature> {
        // The trait is synchronous but the KMS call is async. We bridge by
        // blocking on the current runtime. This is safe because the trait
        // is called from handler code that is already running inside tokio,
        // and `block_in_place` lets the runtime schedule other work during
        // the network round-trip.
        let message_b64 = URL_SAFE_NO_PAD.encode(message);
        let url = format!("{}/kms/v1/keys/{}/sign", self.base_url, self.key_id);
        let http = self.http.clone();
        let body = SignBody { message_b64: &message_b64 };
        let auth_token = self.auth_token.clone();

        let sig_b64 = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let resp = http
                    .post(&url)
                    .bearer_auth(&auth_token)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| MaatError::Crypto(format!("KMS unreachable: {}", e)))?;

                if !resp.status().is_success() {
                    return Err(MaatError::Crypto(format!(
                        "KMS returned {} for sign",
                        resp.status()
                    )));
                }

                let sr: SignResponse = resp
                    .json()
                    .await
                    .map_err(|e| MaatError::Crypto(format!("KMS response parse: {}", e)))?;
                Ok::<String, MaatError>(sr.signature_b64)
            })
        })?;

        let sig_bytes = URL_SAFE_NO_PAD
            .decode(&sig_b64)
            .map_err(|e| MaatError::Crypto(format!("bad signature b64: {}", e)))?;

        Ok(Signature {
            algorithm: SignatureAlgorithm::Ed25519,
            value: sig_bytes,
        })
    }
}
