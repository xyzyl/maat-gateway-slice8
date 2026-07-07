//! Maat KMS daemon entry point.
//!
//! Subcommands:
//!   maat-kmsd serve                    Start the KMS HTTP service (default)
//!   maat-kmsd generate-master-key      Print a fresh 32-byte master key (base64url)
//!   maat-kmsd help                     Print usage

use std::net::SocketAddr;
use std::sync::Arc;

use maat_kms::{
    generate_auth_token_b64, generate_master_key_b64, router, AppState, Config, Vault,
};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging first so startup messages are visible.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "maat_kms=info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let subcommand = args.get(1).map(String::as_str).unwrap_or("serve");

    match subcommand {
        "generate-master-key" => {
            let key = generate_master_key_b64();
            println!("{}", key);
            eprintln!();
            eprintln!("Keep this key safe. If you lose it, the vault becomes unrecoverable.");
            eprintln!("Set it with:");
            eprintln!("  export MAAT_KMS_MASTER_KEY={}", key);
        }
        "generate-auth-token" => {
            let token = generate_auth_token_b64();
            println!("{}", token);
            eprintln!();
            eprintln!("Set this on EVERY service that talks to the KMS:");
            eprintln!("  export MAAT_KMS_AUTH_TOKEN={}", token);
            eprintln!();
            eprintln!("Required by: maat-kmsd, maat-gatewayd, maat-dashboardd,");
            eprintln!("maat-gateway-bootstrap. Rotate by restarting the KMS with");
            eprintln!("a new value and updating every caller's env.");
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        "serve" => {
            serve().await?;
        }
        other => {
            eprintln!("Unknown subcommand: {}", other);
            print_usage();
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn serve() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    info!("Starting Maat KMS");
    info!("  Bind address:  {}", config.bind_addr);
    info!("  Vault path:    {}", config.vault_path.display());
    info!("  Auth required: yes (bearer token, length {})", config.auth_token.len());

    let vault = Vault::open(&config.vault_path, config.master_key).await?;
    let state = Arc::new(AppState {
        vault,
        auth_token: config.auth_token,
    });

    let app = router(state);
    let addr: SocketAddr = config.bind_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Maat KMS listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

fn print_usage() {
    eprintln!("maat-kmsd — Maat Key Management Service");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  maat-kmsd [SUBCOMMAND]");
    eprintln!();
    eprintln!("SUBCOMMANDS:");
    eprintln!("  serve                     Run the KMS HTTP server (default)");
    eprintln!("  generate-master-key       Print a new 32-byte master key and exit");
    eprintln!("  generate-auth-token       Print a new 32-byte API auth token and exit");
    eprintln!("  help                      Print this help message");
    eprintln!();
    eprintln!("ENVIRONMENT (for `serve`):");
    eprintln!("  MAAT_KMS_BIND            Bind address (default: 127.0.0.1:9090)");
    eprintln!("  MAAT_KMS_VAULT           Vault file path (default: ./kms-vault.json)");
    eprintln!("  MAAT_KMS_MASTER_KEY      32-byte base64url key (REQUIRED)");
    eprintln!("  MAAT_KMS_AUTH_TOKEN      Bearer token for API auth (REQUIRED)");
}
