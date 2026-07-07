//! Slice 5 — multi-instance revocation propagation.
//!
//! The defining test of Slice 5: spawn two gateway instances pointing at
//! the same Redis, create a delegation, verify it through gateway A,
//! revoke through the dashboard, then verify the SAME delegation through
//! gateway B and assert it's rejected with `revoked` within seconds.
//!
//! Gated by both MAAT_GATEWAY_TEST_DATABASE_URL and the existence of a
//! reachable Redis. If Redis is unavailable (subscriber.ping fails),
//! gateway construction errors out, which surfaces as a panic in setup.

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

/// Spin up a second gateway pointing at the same database, KMS, and
/// Redis as the harness's primary gateway. Returns its base URL.
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn revocation_propagates_to_second_gateway() {
    let Some(env) = Env::setup().await else {
        eprintln!("skipping: MAAT_GATEWAY_TEST_DATABASE_URL not set");
        return;
    };
    let database_url = std::env::var("MAAT_GATEWAY_TEST_DATABASE_URL").unwrap();

    // Stand up a second gateway sharing DB + KMS + Redis with the first.
    let (gateway_b_url, _gw_b) = spawn_second_gateway(&database_url, &env.kms_url, &env.kms_auth_token).await;

    let dash = cookie_client();
    login(&env, &dash).await;

    // Create a principal key.
    let pk: serde_json::Value = dash
        .post(format!("{}/dashboard/v1/principal-keys", env.dashboard_url))
        .json(&json!({"name": "primary"}))
        .send().await.unwrap()
        .json().await.unwrap();
    let principal_key_id = pk["id"].as_str().unwrap();

    // Create an agent and a delegation good for an hour.
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
        }))
        .send().await.unwrap()
        .json().await.unwrap();
    let delegation_id_b64 = dv["id_b64"].as_str().unwrap().to_string();
    let delegation: maat::Delegation = serde_json::from_value(dv["delegation"].clone()).unwrap();

    // Verify successfully through gateway A.
    let make_request = || {
        let anchor = maat::Anchor::builder(delegation.id.clone())
            .max_staleness(300)
            .contingency(maat::Contingency::Abort)
            .build(&agent)
            .unwrap();
        json!({
            "delegation_chain": [delegation.clone()],
            "anchor": anchor,
            "action_scope": "test:action",
        })
    };

    let resp_a = reqwest::Client::new()
        .post(format!("{}/v1/verify", env.gateway_url))
        .bearer_auth(&env.api_key)
        .json(&make_request())
        .send().await.unwrap();
    assert_eq!(resp_a.status(), 200, "gateway A should accept before revocation");

    // Revoke through the dashboard. This publishes a Redis event.
    let revoke_resp = dash
        .post(format!(
            "{}/dashboard/v1/delegations/{}/revoke",
            env.dashboard_url, delegation_id_b64
        ))
        .json(&json!({"reason": "test"}))
        .send().await.unwrap();
    assert_eq!(revoke_resp.status(), 204, "revoke should succeed");

    // Wait for the propagation to land in gateway B's cache. We poll for
    // up to ~3 seconds rather than sleeping a fixed interval — this keeps
    // the test fast in the common case (low milliseconds in practice).
    let mut last_status = 0u16;
    for _ in 0..30 {
        let resp_b = reqwest::Client::new()
            .post(format!("{}/v1/verify", gateway_b_url))
            .bearer_auth(&env.api_key)
            .json(&make_request())
            .send().await.unwrap();
        last_status = resp_b.status().as_u16();
        if last_status == 403 {
            // Confirm the reason is "revoked" specifically.
            let v: serde_json::Value = resp_b.json().await.unwrap();
            assert_eq!(v["reason"], "revoked",
                "gateway B should reject with reason=revoked");
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!(
        "gateway B did not start rejecting the revoked delegation within 3s; last status was {}",
        last_status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn warm_up_loads_existing_revocations_at_startup() {
    // This test proves that a gateway started AFTER a revocation already
    // happened picks it up via the database warm-up path, not via Redis.
    let Some(env) = Env::setup().await else { return; };
    let database_url = std::env::var("MAAT_GATEWAY_TEST_DATABASE_URL").unwrap();

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
        }))
        .send().await.unwrap()
        .json().await.unwrap();
    let delegation_id_b64 = dv["id_b64"].as_str().unwrap().to_string();
    let delegation: maat::Delegation = serde_json::from_value(dv["delegation"].clone()).unwrap();

    // Revoke FIRST.
    dash.post(format!(
        "{}/dashboard/v1/delegations/{}/revoke",
        env.dashboard_url, delegation_id_b64
    ))
    .json(&json!({"reason": "test"}))
    .send().await.unwrap();

    // THEN spawn a brand-new gateway. Its warm-up should load this
    // revocation from the database before serving any traffic.
    let (gateway_b_url, _gw_b) = spawn_second_gateway(&database_url, &env.kms_url, &env.kms_auth_token).await;

    let anchor = maat::Anchor::builder(delegation.id.clone())
        .max_staleness(300)
        .contingency(maat::Contingency::Abort)
        .build(&agent)
        .unwrap();
    let body = json!({
        "delegation_chain": [delegation],
        "anchor": anchor,
        "action_scope": "test:action",
    });

    let resp = reqwest::Client::new()
        .post(format!("{}/v1/verify", gateway_b_url))
        .bearer_auth(&env.api_key)
        .json(&body)
        .send().await.unwrap();

    assert_eq!(resp.status(), 403, "fresh gateway must reject already-revoked delegation");
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["reason"], "revoked");
}
