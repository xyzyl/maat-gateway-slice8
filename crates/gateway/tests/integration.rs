//! Slice 4 integration tests — multi-tenant verification + cross-tenant isolation.
//!
//! Gated by MAAT_GATEWAY_TEST_DATABASE_URL. Apply both migrations to the
//! target database before running.

mod common;

use common::TestEnv;
use maat::scope::ScopeExpr;
use maat::{Anchor, Contingency, Delegation, Keypair, Receipt};
use serde_json::json;

fn build_valid_request() -> serde_json::Value {
    let now = maat::types::now().unwrap();
    let principal = Keypair::generate();
    let agent = Keypair::generate();
    let agent_pk = agent.public_key.clone();

    let d = Delegation::builder(agent_pk, ScopeExpr::new(vec!["test:action".into()]))
        .not_before(now)
        .not_after(now + 3600)
        .build(&principal)
        .unwrap();

    let anchor = Anchor::builder(d.id.clone())
        .max_staleness(300)
        .contingency(Contingency::Abort)
        .build(&agent)
        .unwrap();

    json!({
        "delegation_chain": [d],
        "anchor": anchor,
        "action_scope": "test:action",
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_endpoint_needs_no_auth() {
    let Some(env) = TestEnv::setup().await else {
        eprintln!("skipping: MAAT_GATEWAY_TEST_DATABASE_URL not set");
        return;
    };
    let r = reqwest::get(format!("{}/v1/health", env.gateway_url)).await.unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn verify_requires_api_key() {
    let Some(env) = TestEnv::setup().await else {
        eprintln!("skipping: MAAT_GATEWAY_TEST_DATABASE_URL not set");
        return;
    };
    let body = build_valid_request();
    let r = reqwest::Client::new()
        .post(format!("{}/v1/verify", env.gateway_url))
        .json(&body)
        .send().await.unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn verify_rejects_invalid_api_key() {
    let Some(env) = TestEnv::setup().await else {
        eprintln!("skipping: MAAT_GATEWAY_TEST_DATABASE_URL not set");
        return;
    };
    let body = build_valid_request();
    let r = reqwest::Client::new()
        .post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth("mgw_live_bogus12345678901234567890")
        .json(&body)
        .send().await.unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn valid_key_authenticates_and_verifies() {
    let Some(env) = TestEnv::setup().await else {
        eprintln!("skipping: MAAT_GATEWAY_TEST_DATABASE_URL not set");
        return;
    };
    let body = build_valid_request();
    let resp = reqwest::Client::new()
        .post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth(&env.tenant_a_api_key)
        .json(&body)
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);

    let v: serde_json::Value = resp.json().await.unwrap();
    let receipt: Receipt = serde_json::from_value(v["receipt"].clone()).unwrap();
    receipt.verify_signature().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn receipts_are_tenant_isolated_in_listings() {
    let Some(env) = TestEnv::setup().await else {
        eprintln!("skipping: MAAT_GATEWAY_TEST_DATABASE_URL not set");
        return;
    };
    let c = reqwest::Client::new();

    let body_a = build_valid_request();
    c.post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth(&env.tenant_a_api_key)
        .json(&body_a)
        .send().await.unwrap();

    let (_tenant_b_id, tenant_b_key) = env.create_second_tenant().await;
    let body_b = build_valid_request();
    c.post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth(&tenant_b_key)
        .json(&body_b)
        .send().await.unwrap();

    let a_list: serde_json::Value = c
        .get(format!("{}/v1/receipts", env.gateway_url))
        .bearer_auth(&env.tenant_a_api_key)
        .send().await.unwrap().json().await.unwrap();

    let b_list: serde_json::Value = c
        .get(format!("{}/v1/receipts", env.gateway_url))
        .bearer_auth(&tenant_b_key)
        .send().await.unwrap().json().await.unwrap();

    let a_receipts = a_list["receipts"].as_array().unwrap();
    let b_receipts = b_list["receipts"].as_array().unwrap();

    assert_eq!(a_receipts.len(), 1, "tenant A should see exactly its own receipt");
    assert_eq!(b_receipts.len(), 1, "tenant B should see exactly its own receipt");

    let a_id = a_receipts[0]["id"].as_str().unwrap();
    let b_id = b_receipts[0]["id"].as_str().unwrap();
    assert_ne!(a_id, b_id);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cannot_fetch_other_tenants_receipt_by_id() {
    let Some(env) = TestEnv::setup().await else {
        eprintln!("skipping: MAAT_GATEWAY_TEST_DATABASE_URL not set");
        return;
    };
    let c = reqwest::Client::new();

    let body_a = build_valid_request();
    let a_resp: serde_json::Value = c
        .post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth(&env.tenant_a_api_key)
        .json(&body_a)
        .send().await.unwrap().json().await.unwrap();
    let a_receipt_id = a_resp["receipt"]["id"].as_str().unwrap().to_string();

    let a_get = c
        .get(format!("{}/v1/receipts/{}", env.gateway_url, a_receipt_id))
        .bearer_auth(&env.tenant_a_api_key)
        .send().await.unwrap();
    assert_eq!(a_get.status(), 200);

    let (_b_id, b_key) = env.create_second_tenant().await;
    let b_get = c
        .get(format!("{}/v1/receipts/{}", env.gateway_url, a_receipt_id))
        .bearer_auth(&b_key)
        .send().await.unwrap();
    assert_eq!(b_get.status(), 404, "Tenant B must not see Tenant A's receipt");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn receipts_signed_by_per_tenant_executor_keys() {
    let Some(env) = TestEnv::setup().await else {
        eprintln!("skipping: MAAT_GATEWAY_TEST_DATABASE_URL not set");
        return;
    };
    let c = reqwest::Client::new();

    let body_a = build_valid_request();
    let a_resp: serde_json::Value = c
        .post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth(&env.tenant_a_api_key)
        .json(&body_a)
        .send().await.unwrap().json().await.unwrap();
    let a_receipt: Receipt = serde_json::from_value(a_resp["receipt"].clone()).unwrap();

    let (_b_id, b_key) = env.create_second_tenant().await;
    let body_b = build_valid_request();
    let b_resp: serde_json::Value = c
        .post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth(&b_key)
        .json(&body_b)
        .send().await.unwrap().json().await.unwrap();
    let b_receipt: Receipt = serde_json::from_value(b_resp["receipt"].clone()).unwrap();

    a_receipt.verify_signature().unwrap();
    b_receipt.verify_signature().unwrap();

    assert_ne!(
        a_receipt.executor.key_data, b_receipt.executor.key_data,
        "per-tenant executor keys must differ"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expired_delegation_returns_403_with_signed_receipt() {
    let Some(env) = TestEnv::setup().await else {
        eprintln!("skipping: MAAT_GATEWAY_TEST_DATABASE_URL not set");
        return;
    };

    let now = maat::types::now().unwrap();
    let p = Keypair::generate();
    let a = Keypair::generate();
    let a_pk = a.public_key.clone();
    let d = Delegation::builder(a_pk, ScopeExpr::new(vec!["test:action".into()]))
        .not_before(now - 100)
        .not_after(now - 10)
        .build(&p)
        .unwrap();
    let anchor = Anchor::builder(d.id.clone())
        .max_staleness(300)
        .contingency(Contingency::Abort)
        .build(&a)
        .unwrap();

    let body = json!({"delegation_chain": [d], "anchor": anchor, "action_scope": "test:action"});

    let r = reqwest::Client::new()
        .post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth(&env.tenant_a_api_key)
        .json(&body)
        .send().await.unwrap();
    assert_eq!(r.status(), 403);
    let v: serde_json::Value = r.json().await.unwrap();
    assert_eq!(v["reason"], "expired");
    let receipt: Receipt = serde_json::from_value(v["receipt"].clone()).unwrap();
    receipt.verify_signature().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revoked_api_key_is_rejected() {
    let Some(env) = TestEnv::setup().await else {
        eprintln!("skipping: MAAT_GATEWAY_TEST_DATABASE_URL not set");
        return;
    };

    let api_keys = maat_config::ApiKeyRepo::new(env.config_pool.clone());
    let doomed = api_keys.create(env.tenant_a_id, "doomed").await.unwrap();
    api_keys
        .revoke(env.tenant_a_id, doomed.metadata.id)
        .await
        .unwrap();

    let body = build_valid_request();
    let r = reqwest::Client::new()
        .post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth(&doomed.full_key)
        .json(&body)
        .send().await.unwrap();
    assert_eq!(r.status(), 401);
}
