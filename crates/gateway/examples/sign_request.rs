//! Build a `/v1/verify` request from a server-signed delegation and an
//! agent secret seed.
//!
//! Usage:
//!     cargo run -p maat-gateway --example sign_request -- \
//!         --delegation delegation.json \
//!         --seed agent_seed.txt \
//!         --scope test:action \
//!         [--out request.json]
//!
//! - delegation.json: the `delegation` field returned by
//!   POST /dashboard/v1/delegations (just the delegation object itself,
//!   NOT the wrapping {id_b64, delegation, ...} envelope).
//! - agent_seed.txt: produced by `generate_agent_key`.
//! - scope: the action scope to claim (must be contained in the
//!   delegation's grants).
//!
//! Each invocation produces a FRESH anchor with a unique nonce, so you
//! can call this between replays to avoid hitting replay detection.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use maat::{Anchor, Contingency, Delegation, Keypair};
use serde_json::json;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut delegation_path: Option<String> = None;
    let mut seed_path: Option<String> = None;
    let mut scope: Option<String> = None;
    let mut out_path = "request.json".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--delegation" => {
                delegation_path = Some(args.get(i + 1).cloned().unwrap_or_default());
                i += 2;
            }
            "--seed" => {
                seed_path = Some(args.get(i + 1).cloned().unwrap_or_default());
                i += 2;
            }
            "--scope" => {
                scope = Some(args.get(i + 1).cloned().unwrap_or_default());
                i += 2;
            }
            "--out" => {
                out_path = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            other => {
                eprintln!("unknown arg: {}", other);
                print_usage();
                std::process::exit(1);
            }
        }
    }

    let delegation_path = delegation_path.unwrap_or_else(|| {
        print_usage();
        std::process::exit(1);
    });
    let seed_path = seed_path.unwrap_or_else(|| {
        print_usage();
        std::process::exit(1);
    });
    let scope = scope.unwrap_or_else(|| {
        print_usage();
        std::process::exit(1);
    });

    // Load and parse the server-signed delegation.
    let del_bytes = std::fs::read(&delegation_path)
        .map_err(|e| anyhow::anyhow!("read {}: {}", delegation_path, e))?;
    let delegation: Delegation = serde_json::from_slice(&del_bytes)
        .map_err(|e| anyhow::anyhow!("parse delegation: {}", e))?;

    // Load the agent seed.
    let seed_b64 = std::fs::read_to_string(&seed_path)
        .map_err(|e| anyhow::anyhow!("read {}: {}", seed_path, e))?;
    let seed_bytes = URL_SAFE_NO_PAD
        .decode(seed_b64.trim())
        .map_err(|e| anyhow::anyhow!("decode seed: {}", e))?;
    if seed_bytes.len() != 32 {
        anyhow::bail!("seed must be 32 bytes, got {}", seed_bytes.len());
    }
    let mut seed_arr = [0u8; 32];
    seed_arr.copy_from_slice(&seed_bytes);
    let agent = Keypair::from_seed(seed_arr);

    // Sanity check: the seed's public key must match the delegation's agent.
    if agent.public_key.key_data != delegation.agent.key_data {
        anyhow::bail!(
            "agent seed does not match delegation's agent public key\n\
             — make sure you used `agent_public.txt` when creating the delegation"
        );
    }

    // Build a fresh anchor signed by the agent.
    let anchor = Anchor::builder(delegation.id.clone())
        .max_staleness(300)
        .contingency(Contingency::Abort)
        .build(&agent)
        .map_err(|e| anyhow::anyhow!("anchor build: {}", e))?;

    let body = json!({
        "delegation_chain": [delegation],
        "anchor": anchor,
        "action_scope": scope,
        "action_description": "request from sign_request example",
    });

    let pretty = serde_json::to_string_pretty(&body)?;
    std::fs::write(&out_path, &pretty)?;

    println!("Wrote {} ({} bytes)", out_path, pretty.len());
    Ok(())
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!(
        "  cargo run -p maat-gateway --example sign_request -- \\\n     \
         --delegation <delegation.json> --seed <agent_seed.txt> --scope <action_scope> \\\n     \
         [--out request.json]"
    );
}
