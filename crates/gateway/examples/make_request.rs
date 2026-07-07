//! Build a valid `/v1/verify` request body and write it to `request.json`.
//!
//! The verification service requires a properly signed Maat delegation and
//! anchor. This example mints fresh keypairs, builds a valid delegation
//! chain of length 1, and serializes the whole bundle ready for curl.
//!
//! Usage:
//!     cargo run -p maat-gateway --example make_request
//!
//! Then:
//!     curl -X POST http://localhost:8080/v1/verify \
//!          -H "Authorization: Bearer mgw_live_<your key>" \
//!          -H "Content-Type: application/json" \
//!          -d @request.json
//!
//! Each run produces a fresh request with new keys and a new nonce, so
//! replay detection won't reject the second curl call. Re-run this between
//! requests if you want to test repeatedly.

use maat::scope::ScopeExpr;
use maat::{Anchor, Contingency, Delegation, Keypair};
use serde_json::json;

fn main() -> anyhow::Result<()> {
    let now = maat::types::now()?;

    let principal = Keypair::generate();
    let agent = Keypair::generate();

    let delegation = Delegation::builder(
        agent.public_key.clone(),
        ScopeExpr::new(vec!["test:action".into()]),
    )
    .not_before(now)
    .not_after(now + 3600)
    .build(&principal)?;

    let anchor = Anchor::builder(delegation.id.clone())
        .max_staleness(300)
        .contingency(Contingency::Abort)
        .build(&agent)?;

    let body = json!({
        "delegation_chain": [delegation],
        "anchor": anchor,
        "action_scope": "test:action",
        "action_description": "example request from make_request.rs",
    });

    let pretty = serde_json::to_string_pretty(&body)?;
    std::fs::write("request.json", &pretty)?;

    println!("Wrote request.json ({} bytes)", pretty.len());
    println!();
    println!("Now run:");
    println!("  curl -X POST http://localhost:8080/v1/verify \\");
    println!("       -H \"Authorization: Bearer mgw_live_<your key>\" \\");
    println!("       -H \"Content-Type: application/json\" \\");
    println!("       -d @request.json");
    Ok(())
}
