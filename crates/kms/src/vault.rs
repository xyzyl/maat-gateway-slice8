//! Encrypted key vault.
//!
//! The vault is a single JSON file on disk. It contains a list of key
//! records, each with a key ID, a creation timestamp, and an Ed25519 seed
//! encrypted under the KMS master key using AES-256-GCM (authenticated
//! encryption).
//!
//! The master key itself is NOT stored in the vault. It is loaded from
//! the environment at startup. If the master key is lost, the vault
//! becomes unrecoverable — this is a feature, not a bug. A compromised
//! vault file without the master key is useless to an attacker.
//!
//! Vault layout on disk (JSON):
//!   {
//!     "version": 1,
//!     "records": [
//!       {
//!         "key_id": "<base64url>",
//!         "created_at": 1767225600,
//!         "algorithm": "ed25519",
//!         "public_key_b64": "<base64url>",
//!         "nonce_b64": "<base64url>",           -- 12 bytes, AES-GCM nonce
//!         "encrypted_seed_b64": "<base64url>"   -- 32-byte seed + 16-byte tag
//!       }
//!     ]
//!   }
//!
//! This format is deliberately simple to audit. A security review can read
//! the entire vault implementation in an afternoon.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use maat::Keypair;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Errors produced by the vault.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("vault i/o error: {0}")]
    Io(String),

    #[error("invalid vault file: {0}")]
    InvalidFormat(String),

    #[error("master key error: {0}")]
    MasterKey(String),

    #[error("encryption error: {0}")]
    Crypto(String),

    #[error("key not found: {0}")]
    KeyNotFound(String),
}

pub type VaultResult<T> = Result<T, VaultError>;

/// A single key record as persisted on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyRecord {
    key_id: String,
    created_at: u64,
    algorithm: String,
    public_key_b64: String,
    /// 12 bytes, base64url-encoded. Unique per record.
    nonce_b64: String,
    /// The 32-byte Ed25519 seed plus AES-GCM's 16-byte authentication tag,
    /// encrypted under the master key. Base64url-encoded.
    encrypted_seed_b64: String,
}

/// On-disk vault file structure.
#[derive(Debug, Serialize, Deserialize)]
struct VaultFile {
    version: u32,
    records: Vec<KeyRecord>,
}

/// The public (non-secret) view of a key. This is safe to return from
/// the KMS API to any caller that can authenticate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKeyView {
    pub key_id: String,
    pub algorithm: String,
    pub public_key_b64: String,
    pub created_at: u64,
}

/// The vault holds all KMS keys. It guards them behind an RwLock so reads
/// (signing) can happen concurrently while writes (key generation) are
/// serialized.
pub struct Vault {
    path: PathBuf,
    master_key: [u8; 32],
    /// In-memory cache of records, keyed by key_id. We keep this so signing
    /// doesn't require reading and parsing the vault file on every request.
    records: Arc<RwLock<HashMap<String, KeyRecord>>>,
}

impl Vault {
    /// Open (or create) a vault at the given path using the provided master
    /// key. The master key must be exactly 32 bytes; the caller is
    /// responsible for deriving it safely (e.g., from an env var via a KDF).
    pub async fn open(path: impl Into<PathBuf>, master_key: [u8; 32]) -> VaultResult<Self> {
        let path = path.into();

        let records = if path.exists() {
            info!("Loading vault from {}", path.display());
            let bytes = tokio::fs::read(&path)
                .await
                .map_err(|e| VaultError::Io(e.to_string()))?;
            let vf: VaultFile = serde_json::from_slice(&bytes)
                .map_err(|e| VaultError::InvalidFormat(e.to_string()))?;
            if vf.version != 1 {
                return Err(VaultError::InvalidFormat(format!(
                    "unsupported vault version {}",
                    vf.version
                )));
            }
            let mut map = HashMap::new();
            for r in vf.records {
                map.insert(r.key_id.clone(), r);
            }
            info!("Loaded {} key record(s)", map.len());
            map
        } else {
            info!("Initializing new vault at {}", path.display());
            HashMap::new()
        };

        let vault = Vault {
            path,
            master_key,
            records: Arc::new(RwLock::new(records)),
        };

        // Verify we can decrypt existing keys (fail fast if master key is wrong).
        vault.verify_master_key().await?;

        // Ensure the file exists with current state.
        vault.flush().await?;

        Ok(vault)
    }

    /// Attempt to decrypt the first key in the vault to confirm the master
    /// key is correct. If the vault is empty, this is a no-op.
    async fn verify_master_key(&self) -> VaultResult<()> {
        let records = self.records.read().await;
        if let Some(record) = records.values().next() {
            self.decrypt_seed(record)?;
            debug!("Master key verified against existing record");
        }
        Ok(())
    }

    /// Generate a new Ed25519 key, store it in the vault, and return the
    /// public view.
    pub async fn generate_key(&self) -> VaultResult<PublicKeyView> {
        // Generate a random seed.
        let mut seed = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut seed);

        // Derive the key_id from the public key. This makes key IDs
        // content-addressed — identical keys would collide, but since we
        // generate fresh seeds, collision is cryptographically impossible.
        let kp = Keypair::from_seed(seed);
        let pub_bytes = &kp.public_key.key_data;

        let mut hasher = Sha256::new();
        hasher.update(b"maat-kms:key-id:v1:");
        hasher.update(pub_bytes);
        let id_bytes = hasher.finalize();
        let key_id = URL_SAFE_NO_PAD.encode(&id_bytes[..16]); // 128-bit ID

        // Encrypt the seed under the master key.
        let (nonce_b64, encrypted_seed_b64) = self.encrypt_seed(&seed)?;

        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let record = KeyRecord {
            key_id: key_id.clone(),
            created_at,
            algorithm: "ed25519".into(),
            public_key_b64: URL_SAFE_NO_PAD.encode(pub_bytes),
            nonce_b64,
            encrypted_seed_b64,
        };

        // Insert and persist.
        {
            let mut records = self.records.write().await;
            records.insert(key_id.clone(), record.clone());
        }
        self.flush().await?;

        info!(
            key_id = %key_id,
            "generated new key"
        );

        Ok(PublicKeyView {
            key_id: record.key_id,
            algorithm: record.algorithm,
            public_key_b64: record.public_key_b64,
            created_at: record.created_at,
        })
    }

    /// Fetch the public view of a key by ID.
    pub async fn get_public(&self, key_id: &str) -> VaultResult<PublicKeyView> {
        let records = self.records.read().await;
        let r = records
            .get(key_id)
            .ok_or_else(|| VaultError::KeyNotFound(key_id.to_string()))?;
        Ok(PublicKeyView {
            key_id: r.key_id.clone(),
            algorithm: r.algorithm.clone(),
            public_key_b64: r.public_key_b64.clone(),
            created_at: r.created_at,
        })
    }

    /// Sign arbitrary bytes with the key identified by `key_id`.
    /// This is the hot path for the KMS — it must be fast.
    pub async fn sign(&self, key_id: &str, message: &[u8]) -> VaultResult<Vec<u8>> {
        let records = self.records.read().await;
        let record = records
            .get(key_id)
            .ok_or_else(|| VaultError::KeyNotFound(key_id.to_string()))?
            .clone();
        drop(records); // release the lock before doing CPU-bound crypto

        let seed = self.decrypt_seed(&record)?;
        let kp = Keypair::from_seed(seed);

        use maat::Signer as MaatSigner;
        let sig = MaatSigner::sign(&kp, message)
            .map_err(|e| VaultError::Crypto(e.to_string()))?;

        Ok(sig.value)
    }

    /// Encrypt a 32-byte seed under the master key.
    /// Returns (nonce_b64, ciphertext_b64).
    fn encrypt_seed(&self, seed: &[u8; 32]) -> VaultResult<(String, String)> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.master_key));

        // AES-GCM nonces must NEVER be reused with the same key.
        // We generate a fresh random 12-byte nonce per encryption.
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, seed.as_ref())
            .map_err(|e| VaultError::Crypto(format!("encrypt failed: {}", e)))?;

        Ok((
            URL_SAFE_NO_PAD.encode(nonce_bytes),
            URL_SAFE_NO_PAD.encode(&ciphertext),
        ))
    }

    /// Decrypt a record's seed under the master key.
    fn decrypt_seed(&self, record: &KeyRecord) -> VaultResult<[u8; 32]> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.master_key));

        let nonce_bytes = URL_SAFE_NO_PAD
            .decode(&record.nonce_b64)
            .map_err(|e| VaultError::InvalidFormat(format!("bad nonce b64: {}", e)))?;
        if nonce_bytes.len() != 12 {
            return Err(VaultError::InvalidFormat("nonce must be 12 bytes".into()));
        }
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = URL_SAFE_NO_PAD
            .decode(&record.encrypted_seed_b64)
            .map_err(|e| VaultError::InvalidFormat(format!("bad ciphertext b64: {}", e)))?;

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| {
                // Do not leak the underlying error — it can reveal whether
                // the master key is wrong vs the ciphertext was tampered.
                VaultError::MasterKey(
                    "decryption failed — wrong master key or vault corrupted".into(),
                )
            })?;

        if plaintext.len() != 32 {
            return Err(VaultError::InvalidFormat(format!(
                "expected 32-byte seed, got {}",
                plaintext.len()
            )));
        }

        let mut seed = [0u8; 32];
        seed.copy_from_slice(&plaintext);
        Ok(seed)
    }

    /// Persist the in-memory records to disk atomically.
    async fn flush(&self) -> VaultResult<()> {
        let records = self.records.read().await;
        let vf = VaultFile {
            version: 1,
            records: records.values().cloned().collect(),
        };
        let json = serde_json::to_vec_pretty(&vf)
            .map_err(|e| VaultError::InvalidFormat(e.to_string()))?;

        // Write to a temp file then rename — this prevents partial writes
        // from corrupting the vault if the process dies mid-write.
        let tmp = self.path.with_extension("tmp");
        tokio::fs::write(&tmp, json)
            .await
            .map_err(|e| VaultError::Io(e.to_string()))?;
        tokio::fs::rename(&tmp, &self.path)
            .await
            .map_err(|e| VaultError::Io(e.to_string()))?;

        Ok(())
    }

    /// List all key IDs in the vault. Used by the API's diagnostic endpoint.
    pub async fn list_ids(&self) -> Vec<String> {
        let records = self.records.read().await;
        records.keys().cloned().collect()
    }
}

/// Parse a master key from a base64url-encoded string (URL-safe, unpadded).
/// The decoded key must be exactly 32 bytes.
pub fn parse_master_key(b64: &str) -> VaultResult<[u8; 32]> {
    let bytes = URL_SAFE_NO_PAD
        .decode(b64)
        .map_err(|e| VaultError::MasterKey(format!("invalid base64: {}", e)))?;
    if bytes.len() != 32 {
        return Err(VaultError::MasterKey(format!(
            "master key must be 32 bytes, got {}",
            bytes.len()
        )));
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(&bytes);
    Ok(k)
}

/// Generate a random 32-byte master key and return it base64url-encoded.
/// Used by the `maat-kmsd generate-master-key` subcommand and by tests.
pub fn generate_master_key_b64() -> String {
    let mut k = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut k);
    URL_SAFE_NO_PAD.encode(k)
}
