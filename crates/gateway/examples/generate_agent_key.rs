//! Mint a fresh Ed25519 agent keypair and save both halves to disk.
//!
//! Usage:
//!     cargo run -p maat-gateway --example generate_agent_key
//!
//! Writes two files to the current directory:
//!   agent_seed.txt     — 32-byte secret seed, base64url-encoded
//!   agent_public.txt   — 32-byte public key, base64url-encoded
//!
//! The seed file is SECRET — treat it like any other private key.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use maat::Keypair;
use rand::RngCore;

fn main() -> anyhow::Result<()> {
    let mut seed = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut seed);

    let keypair = Keypair::from_seed(seed);

    let seed_b64 = URL_SAFE_NO_PAD.encode(seed);
    let public_b64 = URL_SAFE_NO_PAD.encode(&keypair.public_key.key_data);

    std::fs::write("agent_seed.txt", &seed_b64)?;
    std::fs::write("agent_public.txt", &public_b64)?;

    println!("Wrote agent_seed.txt and agent_public.txt");
    println!();
    println!("Public key (give this to the dashboard when creating a delegation):");
    println!("  {}", public_b64);
    println!();
    println!("Secret seed (KEEP THIS SAFE — it's the agent's private key):");
    println!("  {}", seed_b64);
    Ok(())
}
