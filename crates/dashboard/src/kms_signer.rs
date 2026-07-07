//! Adapter that lets a KMS-stored key act as a `maat::Signer`.
//!
//! `Delegation::builder` and `Anchor::builder` (and friends) all take
//! `&impl Signer`. This adapter wraps a KMS HTTP client + key_id +
//! pre-fetched public key so we can pass `&KmsSigner` to the builders.
//!
//! Public key is supplied by the caller (we look it up once when the
//! principal key is created and cache it in the database).

use std::sync::Arc;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use maat::{MaatError, PublicKey, Result as MaatResult, Signature, SignatureAlgorithm, Signer};

use crate::kms_client::KmsHttpClient;

pub struct KmsSigner {
    kms: Arc<KmsHttpClient>,
    key_id: String,
    public_key: PublicKey,
}

impl KmsSigner {
    /// Construct a signer for a known KMS key whose public key we already have.
    pub fn new(
        kms: Arc<KmsHttpClient>,
        key_id: impl Into<String>,
        public_key_b64: &str,
    ) -> Result<Self, String> {
        let key_data = URL_SAFE_NO_PAD
            .decode(public_key_b64)
            .map_err(|e| format!("invalid public_key_b64: {}", e))?;

        Ok(KmsSigner {
            kms,
            key_id: key_id.into(),
            public_key: PublicKey {
                algorithm: SignatureAlgorithm::Ed25519,
                key_data,
            },
        })
    }
}

impl Signer for KmsSigner {
    fn public_key(&self) -> PublicKey {
        self.public_key.clone()
    }

    fn sign(&self, message: &[u8]) -> MaatResult<Signature> {
        let kms = self.kms.clone();
        let key_id = self.key_id.clone();
        let message = message.to_vec();

        // Bridge sync trait to async HTTP call. Same pattern as the
        // gateway's KmsClient::sign.
        let sig_bytes = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                kms.sign(&key_id, &message)
                    .await
                    .map_err(|e| MaatError::Crypto(format!("KMS sign failed: {}", e)))
            })
        })?;

        Ok(Signature {
            algorithm: SignatureAlgorithm::Ed25519,
            value: sig_bytes,
        })
    }
}
