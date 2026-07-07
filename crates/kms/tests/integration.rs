//! End-to-end tests for the KMS.

use std::sync::Arc;
use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use maat_kms::{generate_master_key_b64, parse_master_key, router, AppState, Vault};
use serde_json::json;

async fn spawn_test_kms() -> (String, String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{}", port);

    let tmp = tempfile::tempdir().unwrap().keep();
    let vault_path = tmp.join("kms-vault.json");
    let master_key = parse_master_key(&generate_master_key_b64()).unwrap();

    let vault = Vault::open(&vault_path, master_key).await.unwrap();
    let auth_token = maat_kms::generate_auth_token_b64();
    let state = Arc::new(AppState {
        vault,
        auth_token: auth_token.clone(),
    });
    let app = router(state);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    (base_url, auth_token, handle)
}

#[tokio::test]
async fn health_check_works() {
    let (base, token, _h) = spawn_test_kms().await;
    let r = reqwest::Client::new().get(format!("{}/kms/v1/health", base)).bearer_auth(&token).send().await.unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn generate_key_returns_public_view() {
    let (base, token, _h) = spawn_test_kms().await;
    let c = reqwest::Client::new();

    let resp = c.post(format!("{}/kms/v1/keys", base))
        .bearer_auth(&token).send().await.unwrap();
    assert_eq!(resp.status(), 200);

    let v: serde_json::Value = resp.json().await.unwrap();
    assert!(v["key_id"].is_string());
    assert_eq!(v["algorithm"], "ed25519");
    assert!(v["public_key_b64"].is_string());

    // Public key should decode to exactly 32 bytes.
    let pk_b64 = v["public_key_b64"].as_str().unwrap();
    let pk = URL_SAFE_NO_PAD.decode(pk_b64).unwrap();
    assert_eq!(pk.len(), 32);
}

#[tokio::test]
async fn get_key_returns_same_view_as_generate() {
    let (base, token, _h) = spawn_test_kms().await;
    let c = reqwest::Client::new();

    let gen: serde_json::Value = c
        .post(format!("{}/kms/v1/keys", base))
            .bearer_auth(&token)
        .send().await.unwrap()
        .json().await.unwrap();
    let key_id = gen["key_id"].as_str().unwrap();

    let got: serde_json::Value = c
        .get(format!("{}/kms/v1/keys/{}", base, key_id))
            .bearer_auth(&token)
        .send().await.unwrap()
        .json().await.unwrap();

    assert_eq!(got["key_id"], gen["key_id"]);
    assert_eq!(got["public_key_b64"], gen["public_key_b64"]);
}

#[tokio::test]
async fn get_key_returns_404_for_unknown() {
    let (base, token, _h) = spawn_test_kms().await;
    let r = reqwest::Client::new().get(format!("{}/kms/v1/keys/nonexistent", base)).bearer_auth(&token).send().await.unwrap();
    assert_eq!(r.status(), 404);
}

#[tokio::test]
async fn sign_produces_valid_ed25519_signature() {
    use maat::{PublicKey, Signature, SignatureAlgorithm};

    let (base, token, _h) = spawn_test_kms().await;
    let c = reqwest::Client::new();

    // Generate a key.
    let gen: serde_json::Value = c
        .post(format!("{}/kms/v1/keys", base))
            .bearer_auth(&token)
        .send().await.unwrap()
        .json().await.unwrap();
    let key_id = gen["key_id"].as_str().unwrap().to_string();
    let pk_bytes = URL_SAFE_NO_PAD
        .decode(gen["public_key_b64"].as_str().unwrap())
        .unwrap();

    // Sign a message.
    let msg = b"test payload for signing";
    let body = json!({ "message_b64": URL_SAFE_NO_PAD.encode(msg) });

    let sig: serde_json::Value = c
        .post(format!("{}/kms/v1/keys/{}/sign", base, key_id))
            .bearer_auth(&token)
        .json(&body)
        .send().await.unwrap()
        .json().await.unwrap();

    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig["signature_b64"].as_str().unwrap())
        .unwrap();
    assert_eq!(sig_bytes.len(), 64);

    // Verify the signature independently using the Maat library.
    let pk = PublicKey {
        algorithm: SignatureAlgorithm::Ed25519,
        key_data: pk_bytes,
    };
    let signature = Signature {
        algorithm: SignatureAlgorithm::Ed25519,
        value: sig_bytes,
    };
    maat::crypto::verify_signature(&pk, msg, &signature)
        .expect("signature must verify against the returned public key");
}

#[tokio::test]
async fn sign_returns_404_for_unknown_key() {
    let (base, token, _h) = spawn_test_kms().await;
    let c = reqwest::Client::new();

    let body = json!({ "message_b64": URL_SAFE_NO_PAD.encode(b"anything") });
    let r = c.post(format!("{}/kms/v1/keys/nonexistent/sign", base))
        .bearer_auth(&token)
        .json(&body)
        .send().await.unwrap();
    assert_eq!(r.status(), 404);
}

#[tokio::test]
async fn vault_persists_across_restarts() {
    // Test directly against the Vault API (not HTTP) to verify on-disk state.
    let tmp = tempfile::tempdir().unwrap().keep();
    let vault_path = tmp.join("vault.json");
    let master_key = parse_master_key(&generate_master_key_b64()).unwrap();

    // First session: create a key.
    let key_id;
    let pub_b64;
    {
        let vault = Vault::open(&vault_path, master_key).await.unwrap();
        let view = vault.generate_key().await.unwrap();
        key_id = view.key_id.clone();
        pub_b64 = view.public_key_b64.clone();
    }

    // Second session: reopen and verify the key is still there.
    {
        let vault = Vault::open(&vault_path, master_key).await.unwrap();
        let view = vault.get_public(&key_id).await.unwrap();
        assert_eq!(view.public_key_b64, pub_b64);

        // And that signing still works (proves the master key decrypted the seed).
        let sig = vault.sign(&key_id, b"hello").await.unwrap();
        assert_eq!(sig.len(), 64);
    }
}

#[tokio::test]
async fn wrong_master_key_rejects_existing_vault() {
    let tmp = tempfile::tempdir().unwrap().keep();
    let vault_path = tmp.join("vault.json");

    let real_key = parse_master_key(&generate_master_key_b64()).unwrap();
    let wrong_key = parse_master_key(&generate_master_key_b64()).unwrap();

    // Populate the vault with one record under the real key.
    {
        let vault = Vault::open(&vault_path, real_key).await.unwrap();
        vault.generate_key().await.unwrap();
    }

    // Try to open with a wrong key â€” must fail during verify_master_key.
    let result = Vault::open(&vault_path, wrong_key).await;
    assert!(result.is_err(), "opening vault with wrong master key must fail");
}

// â”€â”€â”€ Auth â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn protected_endpoints_reject_missing_token() {
    let (base, _token, _h) = spawn_test_kms().await;
    let c = reqwest::Client::new();

    let r = c.post(format!("{}/kms/v1/keys", base)).send().await.unwrap();
    assert_eq!(r.status(), 401);
    let body: serde_json::Value = r.json().await.unwrap();
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn protected_endpoints_reject_wrong_token() {
    let (base, _token, _h) = spawn_test_kms().await;
    let c = reqwest::Client::new();

    let r = c.post(format!("{}/kms/v1/keys", base))
        .bearer_auth("not-the-token")
        .send().await.unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn health_endpoint_does_not_require_auth() {
    let (base, _token, _h) = spawn_test_kms().await;
    let r = reqwest::get(format!("{}/kms/v1/health", base)).await.unwrap();
    assert_eq!(r.status(), 200);
}
