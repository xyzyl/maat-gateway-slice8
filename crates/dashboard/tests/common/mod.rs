//! Cross-service test harness: KMS + Gateway + Dashboard.
//!
//! Each test boots three real services on ephemeral ports, talks to them
//! over real HTTP, and asserts the end-to-end flow works. Gated by
//! MAAT_GATEWAY_TEST_DATABASE_URL.

use std::sync::Arc;
use std::time::Duration;

use maat_config::{TenantRepo, UserRepo, UserRole};
use maat_kms::{
    generate_master_key_b64, parse_master_key, router as kms_router, AppState as KmsState, Vault,
};
use uuid::Uuid;

// Shared across several test binaries; not every binary touches every field.
#[allow(dead_code)]
pub struct Env {
    pub gateway_url: String,
    pub dashboard_url: String,
    pub kms_url: String,
    pub kms_auth_token: String,
    pub pool: sqlx::PgPool,

    pub tenant_id: Uuid,
    pub tenant_slug: String,
    pub admin_email: String,
    pub admin_password: String,
    pub api_key: String,

    _kms: tokio::task::JoinHandle<()>,
    _gw: tokio::task::JoinHandle<()>,
    _dash: tokio::task::JoinHandle<()>,
}

impl Env {
    pub async fn setup() -> Option<Self> {
        let database_url = std::env::var("MAAT_GATEWAY_TEST_DATABASE_URL").ok()?;

        // 1. KMS
        let (kms_url, kms_auth_token, kms_handle) = spawn_kms().await;

        // 2. Tenant + KMS executor key + admin user + API key
        let pool = maat_config::connect(&database_url).await.unwrap();

        let http = reqwest::Client::new();
        let kms_resp: serde_json::Value = http
            .post(format!("{}/kms/v1/keys", kms_url))
            .bearer_auth(&kms_auth_token)
            .send().await.unwrap()
            .json().await.unwrap();
        let executor_key_id = kms_resp["key_id"].as_str().unwrap().to_string();

        let slug = format!("test-{}", Uuid::new_v4().simple());
        let tenants = TenantRepo::new(pool.clone());
        let tenant = tenants
            .create(&slug, "Test Tenant", &executor_key_id)
            .await
            .unwrap();

        let admin_email = format!("admin-{}@test.local", Uuid::new_v4().simple());
        let admin_password = "test-password-1234".to_string();
        let users = UserRepo::new(pool.clone());
        users
            .create(tenant.id, &admin_email, &admin_password, UserRole::Admin)
            .await
            .unwrap();

        let api_keys = maat_config::ApiKeyRepo::new(pool.clone());
        let key = api_keys.create(tenant.id, "test-key").await.unwrap();

        // 3. Gateway
        let (gateway_url, gw_handle) = spawn_gateway(&database_url, &kms_url, &kms_auth_token).await;

        // 4. Dashboard
        let (dashboard_url, dash_handle) = spawn_dashboard(&database_url, &kms_url, &kms_auth_token).await;

        Some(Env {
            gateway_url,
            dashboard_url,
            kms_url,
            kms_auth_token,
            pool,
            tenant_id: tenant.id,
            tenant_slug: slug,
            admin_email,
            admin_password,
            api_key: key.full_key,
            _kms: kms_handle,
            _gw: gw_handle,
            _dash: dash_handle,
        })
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

async fn spawn_gateway(
    database_url: &str,
    kms_url: &str,
    kms_auth_token: &str,
) -> (String, tokio::task::JoinHandle<()>) {
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
        redis_url: test_redis_url(),
    };

    let state = Arc::new(maat_gateway::GatewayState::new(config).await.unwrap());
    let app = maat_gateway::build_router(state);

    let h = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    (url, h)
}

async fn spawn_dashboard(
    database_url: &str,
    kms_url: &str,
    kms_auth_token: &str,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{}", port);

    let config = maat_dashboard::Config {
        bind_addr: format!("127.0.0.1:{}", port),
        database_url: database_url.to_string(),
        kms_url: kms_url.to_string(),
        kms_auth_token: kms_auth_token.to_string(),
        redis_url: test_redis_url(),
        secure_cookies: false,
        cors_allowed_origins: Vec::new(),
    };

    let state = Arc::new(maat_dashboard::DashboardState::new(config).await.unwrap());
    let app = maat_dashboard::build_router(state);

    let h = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    (url, h)
}
