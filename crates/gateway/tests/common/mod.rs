//! Shared test harness for Slice 4 integration tests.
//!
//! Each test gets: a running KMS (ephemeral), Postgres connection (external),
//! a running gateway (ephemeral), at least one tenant, and at least one API
//! key. Tests are gated by MAAT_GATEWAY_TEST_DATABASE_URL.

use std::sync::Arc;
use std::time::Duration;

use maat_config::{ApiKeyRepo, TenantRepo};
use maat_kms::{
    generate_master_key_b64, parse_master_key, router as kms_router, AppState as KmsState, Vault,
};
use uuid::Uuid;

// Shared across several test binaries; not every binary touches every field.
#[allow(dead_code)]
pub struct TestEnv {
    pub gateway_url: String,
    pub kms_url: String,
    pub kms_auth_token: String,
    pub config_pool: sqlx::PgPool,

    pub tenant_a_id: Uuid,
    pub tenant_a_api_key: String,
    pub tenant_a_slug: String,

    _kms_handle: tokio::task::JoinHandle<()>,
    _gw_handle: tokio::task::JoinHandle<()>,
}

impl TestEnv {
    pub async fn setup() -> Option<Self> {
        let database_url = std::env::var("MAAT_GATEWAY_TEST_DATABASE_URL").ok()?;

        let (kms_url, kms_auth_token, kms_handle) = spawn_kms().await;

        let pool = maat_config::connect(&database_url).await.unwrap();

        let http = reqwest::Client::new();
        let kms_resp: serde_json::Value = http
            .post(format!("{}/kms/v1/keys", kms_url))
            .bearer_auth(&kms_auth_token)
            .send().await.unwrap()
            .json().await.unwrap();
        let tenant_a_key_id = kms_resp["key_id"].as_str().unwrap().to_string();

        let slug = format!("test-{}", Uuid::new_v4().simple());
        let tenants = TenantRepo::new(pool.clone());
        let tenant_a = tenants
            .create(&slug, "Test Tenant A", &tenant_a_key_id)
            .await
            .unwrap();

        let api_keys = ApiKeyRepo::new(pool.clone());
        let key = api_keys.create(tenant_a.id, "test-key").await.unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let gateway_url = format!("http://127.0.0.1:{}", port);

        let config = maat_gateway::Config {
            bind_addr: format!("127.0.0.1:{}", port),
            storage: maat_gateway::StorageBackend::Postgres {
                database_url: database_url.clone(),
            },
            kms_url: kms_url.clone(),
            kms_auth_token: kms_auth_token.clone(),
            config_database_url: database_url.clone(),
            redis_url: test_redis_url(),
        };

        let state = Arc::new(maat_gateway::GatewayState::new(config).await.unwrap());
        let app = maat_gateway::build_router(state);

        let gw_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        tokio::time::sleep(Duration::from_millis(200)).await;

        Some(TestEnv {
            gateway_url,
            kms_url,

            kms_auth_token,
            config_pool: pool,
            tenant_a_id: tenant_a.id,
            tenant_a_api_key: key.full_key,
            tenant_a_slug: slug,
            _kms_handle: kms_handle,
            _gw_handle: gw_handle,
        })
    }

    pub async fn create_second_tenant(&self) -> (Uuid, String) {
        let http = reqwest::Client::new();
        let kms_resp: serde_json::Value = http
            .post(format!("{}/kms/v1/keys", self.kms_url))
            .bearer_auth(&self.kms_auth_token)
            .send().await.unwrap()
            .json().await.unwrap();
        let key_id = kms_resp["key_id"].as_str().unwrap().to_string();

        let slug = format!("test-{}", Uuid::new_v4().simple());
        let tenants = TenantRepo::new(self.config_pool.clone());
        let tenant = tenants
            .create(&slug, "Test Tenant B", &key_id)
            .await
            .unwrap();

        let api_keys = ApiKeyRepo::new(self.config_pool.clone());
        let key = api_keys.create(tenant.id, "test-key-b").await.unwrap();

        (tenant.id, key.full_key)
    }
}

/// Redis URL used by Slice 5 tests. Defaults to a localhost Redis but can
/// be overridden via MAAT_REDIS_TEST_URL.
fn test_redis_url() -> String {
    std::env::var("MAAT_REDIS_TEST_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string())
}

async fn spawn_kms() -> (String, String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{}", port);

    let tmp = tempfile::tempdir().unwrap().keep();
    let vault_path = tmp.join("kms-vault.json");
    let master_key = parse_master_key(&generate_master_key_b64()).unwrap();
    let vault = Vault::open(&vault_path, master_key).await.unwrap();
    let auth_token = maat_kms::generate_auth_token_b64();
    let state = Arc::new(KmsState {
        vault,
        auth_token: auth_token.clone(),
    });
    let app = kms_router(state);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    (base_url, auth_token, handle)
}
