//! `sec verify` — verify an attested document against the signer's did:web.

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use secretariat_core::application::{verify_document, VerifyOutcome};
use secretariat_core::infrastructure::{CompositeDidResolver, DidWebResolver};

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    /// File to verify.
    file: PathBuf,

    /// Emit machine-readable JSON to stdout (for Claude / MCP / scripts).
    #[arg(long, default_value_t = false)]
    json: bool,
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;

    let resolver = CompositeDidResolver::new(DidWebResolver::new(paths.peers_cache.clone()));
    let outcome = verify_document(&args.file, &resolver)
        .with_context(|| format!("verifying {}", args.file.display()))?;

    if args.json {
        print_json(&outcome);
    } else {
        print_human(&outcome);
    }

    let exit = match outcome {
        VerifyOutcome::Verified { .. } => 0,
        _ => 2,
    };
    if exit != 0 {
        std::process::exit(exit);
    }
    Ok(())
}

fn print_human(out: &VerifyOutcome) {
    match out {
        VerifyOutcome::Verified {
            signer,
            stamped_at,
            act,
        } => {
            println!("✓ attested by {signer} at {stamped_at} (act: {act})");
        }
        VerifyOutcome::Tampered {
            claimed_hash,
            computed_hash,
        } => {
            println!("✗ tampered (claimed {claimed_hash}, computed {computed_hash})");
        }
        VerifyOutcome::Unsigned => {
            println!("✗ unsigned (no $attestation block)");
        }
        VerifyOutcome::SignerUnresolvable { signer, cause } => {
            println!("✗ cannot resolve signer {signer}: {cause}");
        }
        VerifyOutcome::SignatureInvalid { signer } => {
            println!("✗ signature does not verify for signer {signer}");
        }
    }
}

fn print_json(out: &VerifyOutcome) {
    use serde_json::json;
    let v = match out {
        VerifyOutcome::Verified {
            signer,
            stamped_at,
            act,
        } => json!({
            "outcome": "verified",
            "signer": signer.as_str(),
            "stampedAt": stamped_at,
            "act": format!("{act}"),
        }),
        VerifyOutcome::Tampered {
            claimed_hash,
            computed_hash,
        } => json!({
            "outcome": "tampered",
            "claimedHash": claimed_hash.to_string(),
            "computedHash": computed_hash.to_string(),
        }),
        VerifyOutcome::Unsigned => json!({ "outcome": "unsigned" }),
        VerifyOutcome::SignerUnresolvable { signer, cause } => json!({
            "outcome": "signerUnresolvable",
            "signer": signer.as_str(),
            "cause": cause.to_string(),
        }),
        VerifyOutcome::SignatureInvalid { signer } => json!({
            "outcome": "signatureInvalid",
            "signer": signer.as_str(),
        }),
    };
    println!("{}", serde_json::to_string_pretty(&v).unwrap());
}
