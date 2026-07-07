//! End-to-end integration tests for the dashboard service and the
//! cross-service flow (dashboard → gateway via Maat protocol).
//!
//! Gated by MAAT_GATEWAY_TEST_DATABASE_URL.

mod common;

use common::Env;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::json;

fn cookie_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap()
}

async fn login(env: &Env, c: &reqwest::Client) {
    let r = c
        .post(format!("{}/dashboard/v1/auth/login", env.dashboard_url))
        .json(&json!({
            "tenant": env.tenant_slug,
            "email": env.admin_email,
            "password": env.admin_password,
        }))
        .send().await.unwrap();
    assert_eq!(r.status(), 200, "login failed");
}

// ─── Auth ───

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_endpoint_works() {
    let Some(env) = Env::setup().await else {
        eprintln!("skipping: MAAT_GATEWAY_TEST_DATABASE_URL not set");
        return;
    };
    let r = reqwest::get(format!("{}/dashboard/v1/health", env.dashboard_url))
        .await.unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_with_bad_credentials_returns_401() {
    let Some(env) = Env::setup().await else { return; };
    let r = reqwest::Client::new()
        .post(format!("{}/dashboard/v1/auth/login", env.dashboard_url))
        .json(&json!({
            "tenant": env.tenant_slug,
            "email": env.admin_email,
            "password": "wrong",
        }))
        .send().await.unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn me_requires_session() {
    let Some(env) = Env::setup().await else { return; };
    let r = reqwest::get(format!("{}/dashboard/v1/auth/me", env.dashboard_url))
        .await.unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_then_me_returns_user() {
    let Some(env) = Env::setup().await else { return; };
    let c = cookie_client();
    login(&env, &c).await;

    let me: serde_json::Value = c
        .get(format!("{}/dashboard/v1/auth/me", env.dashboard_url))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(me["email"], env.admin_email);
    assert_eq!(me["role"], "admin");
}

// ─── Principal keys ───

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_principal_key_via_dashboard() {
    let Some(env) = Env::setup().await else { return; };
    let c = cookie_client();
    login(&env, &c).await;

    let resp = c
        .post(format!("{}/dashboard/v1/principal-keys", env.dashboard_url))
        .json(&json!({"name": "primary"}))
        .send().await.unwrap();
    assert_eq!(resp.status(), 201);

    let pk: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(pk["name"], "primary");
    assert!(pk["kms_key_id"].is_string());
    assert!(pk["public_key_b64"].is_string());

    // Should appear in the list.
    let list: serde_json::Value = c
        .get(format!("{}/dashboard/v1/principal-keys", env.dashboard_url))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
}

// ─── End-to-end delegation flow ───

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_journey_create_verify_audit() {
    let Some(env) = Env::setup().await else { return; };
    let c = cookie_client();
    login(&env, &c).await;

    // 1. Create a principal key for the tenant.
    let pk: serde_json::Value = c
        .post(format!("{}/dashboard/v1/principal-keys", env.dashboard_url))
        .json(&json!({"name": "primary"}))
        .send().await.unwrap()
        .json().await.unwrap();
    let principal_key_id = pk["id"].as_str().unwrap();

    // 2. Generate an agent keypair locally (the agent owns its own keys).
    let agent = maat::Keypair::generate();
    let agent_pubkey_b64 = URL_SAFE_NO_PAD.encode(&agent.public_key.key_data);

    // 3. Create a delegation through the dashboard. KMS signs it.
    let now = maat::types::now().unwrap();
    let create_resp = c
        .post(format!("{}/dashboard/v1/delegations", env.dashboard_url))
        .json(&json!({
            "principal_key_id": principal_key_id,
            "agent_pubkey_b64": agent_pubkey_b64,
            "scope_grants": ["test:action"],
            "not_before": now,
            "not_after": now + 3600,
        }))
        .send().await.unwrap();
    assert_eq!(create_resp.status(), 201, "delegation creation failed");

    let dv: serde_json::Value = create_resp.json().await.unwrap();
    let delegation: maat::Delegation = serde_json::from_value(dv["delegation"].clone()).unwrap();

    // 4. Build an anchor for the agent.
    let anchor = maat::Anchor::builder(delegation.id.clone())
        .max_staleness(300)
        .contingency(maat::Contingency::Abort)
        .build(&agent)
        .unwrap();

    // 5. Submit the verify request to the gateway.
    let verify_body = json!({
        "delegation_chain": [delegation],
        "anchor": anchor,
        "action_scope": "test:action",
    });

    let verify_resp = reqwest::Client::new()
        .post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth(&env.api_key)
        .json(&verify_body)
        .send().await.unwrap();
    assert_eq!(verify_resp.status(), 200, "gateway verify failed");

    let v: serde_json::Value = verify_resp.json().await.unwrap();
    assert_eq!(v["verified"], true);
    let receipt: maat::Receipt = serde_json::from_value(v["receipt"].clone()).unwrap();
    receipt.verify_signature().unwrap();

    // 6. Verify the receipt is visible through the dashboard.
    let receipts: serde_json::Value = c
        .get(format!("{}/dashboard/v1/receipts", env.dashboard_url))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(receipts["total"], 1);
}

// ─── Cross-tenant isolation through the dashboard ───

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dashboard_cannot_see_other_tenants_data() {
    let Some(env_a) = Env::setup().await else { return; };
    // env_a has its own tenant. Build a SECOND tenant in the same DB.
    let env_b = Env::setup().await.unwrap();

    let c_a = cookie_client();
    login(&env_a, &c_a).await;

    // env_a creates a principal key.
    c_a.post(format!("{}/dashboard/v1/principal-keys", env_a.dashboard_url))
        .json(&json!({"name": "for-a"}))
        .send().await.unwrap();

    let c_b = cookie_client();
    login(&env_b, &c_b).await;

    // env_b lists principal keys and must see zero — env_a's key belongs
    // to env_a's tenant.
    let b_keys: serde_json::Value = c_b
        .get(format!("{}/dashboard/v1/principal-keys", env_b.dashboard_url))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(b_keys.as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn viewer_cannot_create_resources() {
    let Some(env) = Env::setup().await else { return; };

    // Create a viewer user via the bootstrap API path: just call the repo
    // directly (the bootstrap CLI is harder to invoke from a test).
    let viewer_email = format!("viewer-{}@test.local", uuid::Uuid::new_v4().simple());
    let viewer_password = "viewer-password-1234".to_string();
    let users = maat_config::UserRepo::new(env.pool.clone());
    users
        .create(env.tenant_id, &viewer_email, &viewer_password, maat_config::UserRole::Viewer)
        .await
        .unwrap();

    let c = cookie_client();
    let r = c
        .post(format!("{}/dashboard/v1/auth/login", env.dashboard_url))
        .json(&json!({
            "tenant": env.tenant_slug,
            "email": viewer_email,
            "password": viewer_password,
        }))
        .send().await.unwrap();
    assert_eq!(r.status(), 200);

    // Try to create a principal key as a viewer — must be 403.
    let r = c
        .post(format!("{}/dashboard/v1/principal-keys", env.dashboard_url))
        .json(&json!({"name": "should-fail"}))
        .send().await.unwrap();
    assert_eq!(r.status(), 403);
}
