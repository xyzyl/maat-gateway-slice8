//! Bootstrap CLI for the Maat Gateway.
//!
//! Subcommands:
//!   create-tenant <slug> <name>          Provision a new tenant + KMS key
//!   create-api-key <tenant-slug> <name>  Issue an API key (printed once)

use std::process::ExitCode;

use maat_config::{ApiKeyRepo, TenantRepo};

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt().with_target(false).init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_usage();
        return ExitCode::FAILURE;
    }

    let result = match args[1].as_str() {
        "create-tenant" => create_tenant(&args[2..]).await,
        "create-api-key" => create_api_key(&args[2..]).await,
        "create-user" => create_user(&args[2..]).await,
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => {
            eprintln!("Unknown subcommand: {}", other);
            print_usage();
            return ExitCode::FAILURE;
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

async fn create_tenant(args: &[String]) -> anyhow::Result<()> {
    if args.len() < 2 {
        anyhow::bail!("usage: create-tenant <slug> <display-name>");
    }
    let slug = &args[0];
    let name = &args[1];

    let kms_url = std::env::var("MAAT_KMS_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string());
    let kms_auth_token = std::env::var("MAAT_KMS_AUTH_TOKEN").map_err(|_| {
        anyhow::anyhow!(
            "MAAT_KMS_AUTH_TOKEN is required (matches the value the KMS was started with; \
             generate one with `maat-kmsd generate-auth-token`)"
        )
    })?;

    eprintln!("Generating executor key in KMS at {}...", kms_url);
    let http = reqwest::Client::new();
    let resp = http
        .post(format!("{}/kms/v1/keys", kms_url))
        .bearer_auth(&kms_auth_token)
        .send()
        .await?;
    if !resp.status().is_success() {
        anyhow::bail!(
            "KMS key generation returned {} (check MAAT_KMS_AUTH_TOKEN matches the KMS's value)",
            resp.status()
        );
    }
    let kms_resp: serde_json::Value = resp.json().await?;
    let key_id = kms_resp["key_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("KMS response missing key_id"))?
        .to_string();

    let config_url = config_db_url()?;
    let pool = maat_config::connect(&config_url).await?;
    let tenants = TenantRepo::new(pool);

    let tenant = tenants.create(slug, name, &key_id).await?;

    println!("Created tenant:");
    println!("  ID:                    {}", tenant.id);
    println!("  Slug:                  {}", tenant.slug);
    println!("  Name:                  {}", tenant.name);
    println!("  KMS executor key ID:   {}", tenant.kms_executor_key_id);
    println!();
    println!("Next: maat-gateway-bootstrap create-api-key {} <key-name>", tenant.slug);
    Ok(())
}

async fn create_api_key(args: &[String]) -> anyhow::Result<()> {
    if args.len() < 2 {
        anyhow::bail!("usage: create-api-key <tenant-slug> <key-name>");
    }
    let slug = &args[0];
    let key_name = &args[1];

    let config_url = config_db_url()?;
    let pool = maat_config::connect(&config_url).await?;
    let tenants = TenantRepo::new(pool.clone());
    let api_keys = ApiKeyRepo::new(pool);

    let tenant = tenants.get_by_slug(slug).await?;
    let created = api_keys.create(tenant.id, key_name).await?;

    println!("API key created for tenant '{}':", tenant.slug);
    println!();
    println!("  {}", created.full_key);
    println!();
    println!("This is the ONLY time this key will be displayed.");
    println!("Store it somewhere safe. If lost, create a new one and revoke the old.");
    println!();
    println!("Metadata:");
    println!("  Key ID:   {}", created.metadata.id);
    println!("  Name:     {}", created.metadata.name);
    println!("  Prefix:   {}", created.metadata.key_prefix);
    Ok(())
}

async fn create_user(args: &[String]) -> anyhow::Result<()> {
    if args.len() < 4 {
        anyhow::bail!("usage: create-user <tenant-slug> <email> <password> <role>");
    }
    let slug = &args[0];
    let email = &args[1];
    let password = &args[2];
    let role_str = &args[3];

    let role = maat_config::UserRole::parse(role_str)
        .map_err(|_| anyhow::anyhow!("role must be 'admin' or 'viewer'"))?;

    if password.len() < 8 {
        anyhow::bail!("password must be at least 8 characters");
    }

    let config_url = config_db_url()?;
    let pool = maat_config::connect(&config_url).await?;
    let tenants = maat_config::TenantRepo::new(pool.clone());
    let users = maat_config::UserRepo::new(pool);

    let tenant = tenants.get_by_slug(slug).await?;
    let user = users.create(tenant.id, email, password, role).await?;

    println!("Created user:");
    println!("  ID:       {}", user.id);
    println!("  Tenant:   {} ({})", tenant.slug, tenant.id);
    println!("  Email:    {}", user.email);
    println!("  Role:     {}", user.role.as_str());
    Ok(())
}

fn config_db_url() -> anyhow::Result<String> {
    std::env::var("MAAT_CONFIG_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .map_err(|_| anyhow::anyhow!("MAAT_CONFIG_DATABASE_URL (or DATABASE_URL) must be set"))
}

fn print_usage() {
    eprintln!("maat-gateway-bootstrap — administrative CLI for the Maat Gateway");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  maat-gateway-bootstrap <subcommand> [args]");
    eprintln!();
    eprintln!("SUBCOMMANDS:");
    eprintln!("  create-tenant <slug> <display-name>             Provision a new tenant");
    eprintln!("  create-api-key <tenant-slug> <key-name>         Issue an API key for a tenant");
    eprintln!("  create-user <tenant-slug> <email> <password> <role>");
    eprintln!("                                                  Create a dashboard user (role: admin|viewer)");
    eprintln!("  help                                            Print this help");
    eprintln!();
    eprintln!("ENVIRONMENT:");
    eprintln!("  DATABASE_URL or MAAT_CONFIG_DATABASE_URL   Postgres connection URL (required)");
    eprintln!("  MAAT_KMS_URL                               KMS URL (default: http://127.0.0.1:9090)");
    eprintln!("  MAAT_KMS_AUTH_TOKEN                        KMS bearer token (REQUIRED for create-tenant)");
}
