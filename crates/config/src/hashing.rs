//! Password and API key hashing — Argon2id for both.

use argon2::Argon2;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rand::rngs::OsRng;
use rand::RngCore;

use crate::errors::{ConfigError, ConfigResult};

pub fn hash_password(password: &str) -> ConfigResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let hash = argon
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| ConfigError::Hashing(format!("hash failed: {}", e)))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, encoded_hash: &str) -> bool {
    match PasswordHash::new(encoded_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// Generate a new API key. Returns `(full_key, key_prefix, key_hash)`.
pub fn generate_api_key() -> ConfigResult<(String, String, String)> {
    let mut key_bytes = [0u8; 20];
    OsRng.fill_bytes(&mut key_bytes);
    let key_body = URL_SAFE_NO_PAD.encode(key_bytes);
    let full_key = format!("mgw_live_{}", key_body);
    let key_prefix = key_body.chars().take(8).collect::<String>();

    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let hash = argon
        .hash_password(full_key.as_bytes(), &salt)
        .map_err(|e| ConfigError::Hashing(format!("hash failed: {}", e)))?;

    Ok((full_key, key_prefix, hash.to_string()))
}

pub fn api_key_prefix(full_key: &str) -> Option<String> {
    let body = full_key.strip_prefix("mgw_live_")?;
    if body.len() < 8 {
        return None;
    }
    Some(body.chars().take(8).collect())
}

pub fn hash_api_key(full_key: &str) -> ConfigResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let hash = argon
        .hash_password(full_key.as_bytes(), &salt)
        .map_err(|e| ConfigError::Hashing(format!("hash failed: {}", e)))?;
    Ok(hash.to_string())
}

pub fn verify_api_key(full_key: &str, encoded_hash: &str) -> bool {
    match PasswordHash::new(encoded_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(full_key.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_roundtrip() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong", &hash));
    }

    #[test]
    fn api_key_roundtrip() {
        let (full, prefix, hash) = generate_api_key().unwrap();
        assert!(full.starts_with("mgw_live_"));
        assert_eq!(prefix.len(), 8);
        assert!(verify_api_key(&full, &hash));
        assert!(!verify_api_key("mgw_live_bogus", &hash));
    }

    #[test]
    fn prefix_extraction() {
        let (full, prefix, _) = generate_api_key().unwrap();
        assert_eq!(api_key_prefix(&full).unwrap(), prefix);
        assert_eq!(api_key_prefix("not-a-key"), None);
        assert_eq!(api_key_prefix("mgw_live_abc"), None);
    }
}
