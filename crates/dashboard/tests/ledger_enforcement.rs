//! Slice 7 — cross-instance cumulative ledger enforcement.
//!
//! Two gateways pointing at the same Redis and Postgres MUST jointly
//! enforce one cumulative cap. The Redis Lua script provides the
//! atomic check-and-increment; this test asserts the property
//! end-to-end through the HTTP layer.
//!
//! Gated by MAAT_GATEWAY_TEST_DATABASE_URL and a reachable Redis.

mod common;

use std::sync::Arc;
use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use common::Env;
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
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
}

async fn spawn_second_gateway(
    database_url: &str,
    kms_url: &str,
    kms_auth_token: &str,
) -> (String, tokio::task::JoinHandle<()>) {
    let redis_url = std::env::var("MAAT_REDIS_TEST_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{}", port);

    let config = maat_gateway::Config {
        bind_addr: format!("127.0.0.1:{}", port),
        storage: maat_gateway::StorageBackend::Postgres {
            database_url: database_url.to_string(),
        },
        kms_url: kms_url.to_string(),
        kms_auth_token: kms_auth_token.to_string(),
        config_database_url: database_url.to_string(),
        redis_url,
    };

    let state = Arc::new(maat_gateway::GatewayState::new(config).await.unwrap());
    let app = maat_gateway::build_router(state);

    let h = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    (url, h)
}

/// Build a fresh value-bearing verify request for the given delegation.
async fn build_verify_body(
    delegation: &maat::Delegation,
    agent: &maat::Keypair,
    amount_minor: u64,
) -> serde_json::Value {
    let anchor = maat::Anchor::builder(delegation.id.clone())
        .max_staleness(300)
        .contingency(maat::Contingency::Abort)
        .build(agent)
        .unwrap();

    json!({
        "delegation_chain": [delegation],
        "anchor": anchor,
        "action_scope": "test:action",
        "value_claim": {
            "currency": "USD",
            "amount": amount_minor,
            "decimals": 2,
        },
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cumulative_cap_enforced_jointly_across_instances() {
    let Some(env) = Env::setup().await else {
        eprintln!("skipping: MAAT_GATEWAY_TEST_DATABASE_URL not set");
        return;
    };
    let database_url = std::env::var("MAAT_GATEWAY_TEST_DATABASE_URL").unwrap();

    // Stand up gateway B sharing DB + KMS + Redis with gateway A.
    let (gateway_b_url, _gw_b) = spawn_second_gateway(&database_url, &env.kms_url, &env.kms_auth_token).await;

    let dash = cookie_client();
    login(&env, &dash).await;

    // Principal key.
    let pk: serde_json::Value = dash
        .post(format!("{}/dashboard/v1/principal-keys", env.dashboard_url))
        .json(&json!({"name": "primary"}))
        .send().await.unwrap()
        .json().await.unwrap();
    let principal_key_id = pk["id"].as_str().unwrap();

    // Agent + delegation: $50 per action, $200 cumulative.
    let agent = maat::Keypair::generate();
    let agent_pubkey_b64 = URL_SAFE_NO_PAD.encode(&agent.public_key.key_data);
    let now = maat::types::now().unwrap();

    let dv: serde_json::Value = dash
        .post(format!("{}/dashboard/v1/delegations", env.dashboard_url))
        .json(&json!({
            "principal_key_id": principal_key_id,
            "agent_pubkey_b64": agent_pubkey_b64,
            "scope_grants": ["test:action"],
            "not_before": now,
            "not_after": now + 3600,
            "constraints": [
                { "type": "max_value", "currency": "USD", "amount": 5000, "decimals": 2 }
            ],
            "cumulative_cap": {
                "currency": "USD",
                "amount": 20000,    // $200.00
                "decimals": 2
            }
        }))
        .send().await.unwrap()
        .json().await.unwrap();
    let delegation: maat::Delegation = serde_json::from_value(dv["delegation"].clone()).unwrap();

    let http = reqwest::Client::new();

    // Spend $40 four times alternating between gateways A and B.
    // Total after 4 actions = $160; the 5th action of $40 should bring it
    // to exactly $200 and still succeed; the 6th must fail.
    for i in 0..5u32 {
        let url = if i % 2 == 0 { &env.gateway_url } else { &gateway_b_url };
        let body = build_verify_body(&delegation, &agent, 4000).await;
        let resp = http
            .post(format!("{}/v1/verify", url))
            .bearer_auth(&env.api_key)
            .json(&body)
            .send().await.unwrap();
        let status = resp.status();
        let v: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(
            status, 200,
            "request {} should have succeeded, got {} body {:?}",
            i, status, v
        );
        assert_eq!(v["verified"], true);
    }

    // Sixth attempt — running total would be $240, over the $200 cap.
    // Hit gateway B specifically to exercise the cross-instance path.
    let body = build_verify_body(&delegation, &agent, 4000).await;
    let resp = http
        .post(format!("{}/v1/verify", gateway_b_url))
        .bearer_auth(&env.api_key)
        .json(&body)
        .send().await.unwrap();
    let status = resp.status();
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        status, 403,
        "gateway B should have rejected (cap exceeded), got {} body {:?}",
        status, v
    );
    assert_eq!(v["reason"], "constraint_violated");
    let detail = v["detail"].as_str().unwrap_or("");
    assert!(
        detail.contains("cumulative cap"),
        "rejection should mention cumulative cap, got: {}",
        detail
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn per_action_max_value_rejects_oversized_claim() {
    // Sanity check that the protocol's per-action MaxValue still works
    // through the gateway. With cap = $50 per action, a $60 claim is
    // rejected by the protocol library directly, before the ledger.
    let Some(env) = Env::setup().await else { return; };

    let dash = cookie_client();
    login(&env, &dash).await;

    let pk: serde_json::Value = dash
        .post(format!("{}/dashboard/v1/principal-keys", env.dashboard_url))
        .json(&json!({"name": "primary"}))
        .send().await.unwrap()
        .json().await.unwrap();
    let principal_key_id = pk["id"].as_str().unwrap();

    let agent = maat::Keypair::generate();
    let agent_pubkey_b64 = URL_SAFE_NO_PAD.encode(&agent.public_key.key_data);
    let now = maat::types::now().unwrap();

    let dv: serde_json::Value = dash
        .post(format!("{}/dashboard/v1/delegations", env.dashboard_url))
        .json(&json!({
            "principal_key_id": principal_key_id,
            "agent_pubkey_b64": agent_pubkey_b64,
            "scope_grants": ["test:action"],
            "not_before": now,
            "not_after": now + 3600,
            "constraints": [
                { "type": "max_value", "currency": "USD", "amount": 5000, "decimals": 2 }
            ]
        }))
        .send().await.unwrap()
        .json().await.unwrap();
    let delegation: maat::Delegation = serde_json::from_value(dv["delegation"].clone()).unwrap();

    let body = build_verify_body(&delegation, &agent, 6000).await; // $60
    let resp = reqwest::Client::new()
        .post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth(&env.api_key)
        .json(&body)
        .send().await.unwrap();
    assert_eq!(resp.status(), 403);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["reason"], "constraint_violated");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn missing_value_claim_with_max_value_constraint_rejects() {
    // Fail-closed: if the delegation has MaxValue and the verify omits
    // value_claim, the protocol library rejects.
    let Some(env) = Env::setup().await else { return; };

    let dash = cookie_client();
    login(&env, &dash).await;

    let pk: serde_json::Value = dash
        .post(format!("{}/dashboard/v1/principal-keys", env.dashboard_url))
        .json(&json!({"name": "primary"}))
        .send().await.unwrap()
        .json().await.unwrap();
    let principal_key_id = pk["id"].as_str().unwrap();

    let agent = maat::Keypair::generate();
    let agent_pubkey_b64 = URL_SAFE_NO_PAD.encode(&agent.public_key.key_data);
    let now = maat::types::now().unwrap();

    let dv: serde_json::Value = dash
        .post(format!("{}/dashboard/v1/delegations", env.dashboard_url))
        .json(&json!({
            "principal_key_id": principal_key_id,
            "agent_pubkey_b64": agent_pubkey_b64,
            "scope_grants": ["test:action"],
            "not_before": now,
            "not_after": now + 3600,
            "constraints": [
                { "type": "max_value", "currency": "USD", "amount": 5000, "decimals": 2 }
            ]
        }))
        .send().await.unwrap()
        .json().await.unwrap();
    let delegation: maat::Delegation = serde_json::from_value(dv["delegation"].clone()).unwrap();

    let anchor = maat::Anchor::builder(delegation.id.clone())
        .max_staleness(300)
        .contingency(maat::Contingency::Abort)
        .build(&agent)
        .unwrap();

    // No value_claim included.
    let body = json!({
        "delegation_chain": [delegation],
        "anchor": anchor,
        "action_scope": "test:action",
    });

    let resp = reqwest::Client::new()
        .post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth(&env.api_key)
        .json(&body)
        .send().await.unwrap();
    assert_eq!(resp.status(), 403);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["reason"], "constraint_violated");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ledger_summary_endpoint_reports_running_total() {
    let Some(env) = Env::setup().await else { return; };

    let dash = cookie_client();
    login(&env, &dash).await;

    let pk: serde_json::Value = dash
        .post(format!("{}/dashboard/v1/principal-keys", env.dashboard_url))
        .json(&json!({"name": "primary"}))
        .send().await.unwrap()
        .json().await.unwrap();
    let principal_key_id = pk["id"].as_str().unwrap();

    let agent = maat::Keypair::generate();
    let agent_pubkey_b64 = URL_SAFE_NO_PAD.encode(&agent.public_key.key_data);
    let now = maat::types::now().unwrap();

    let dv: serde_json::Value = dash
        .post(format!("{}/dashboard/v1/delegations", env.dashboard_url))
        .json(&json!({
            "principal_key_id": principal_key_id,
            "agent_pubkey_b64": agent_pubkey_b64,
            "scope_grants": ["test:action"],
            "not_before": now,
            "not_after": now + 3600,
            "constraints": [
                { "type": "max_value", "currency": "USD", "amount": 5000, "decimals": 2 }
            ],
            "cumulative_cap": {
                "currency": "USD",
                "amount": 10000,
                "decimals": 2
            }
        }))
        .send().await.unwrap()
        .json().await.unwrap();
    let delegation_id_b64 = dv["id_b64"].as_str().unwrap().to_string();
    let delegation: maat::Delegation = serde_json::from_value(dv["delegation"].clone()).unwrap();

    // Spend $30.
    let http = reqwest::Client::new();
    for _ in 0..3 {
        let body = build_verify_body(&delegation, &agent, 1000).await;
        let r = http
            .post(format!("{}/v1/verify", env.gateway_url))
            .bearer_auth(&env.api_key)
            .json(&body)
            .send().await.unwrap();
        assert_eq!(r.status(), 200);
    }

    // Read the ledger summary.
    let summary: serde_json::Value = dash
        .get(format!(
            "{}/dashboard/v1/delegations/{}/ledger",
            env.dashboard_url, delegation_id_b64
        ))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(summary["cap"]["amount"], 10000);
    assert_eq!(summary["recorded_total"], 3000);
    assert!(summary["recent_entries"].as_array().unwrap().len() >= 3);
}
