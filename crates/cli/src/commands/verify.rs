//! `sec verify` — three-state layered verify of an attested document.
//!
//! Substrate-for-themia Move 13 wires the CLI to the
//! [`verify_document_layered`] use case introduced in Move 2. Output
//! reports the author signature (`$signature`) and the principal stamp
//! (`$attestation`) as independent layers, each with one of three
//! states: ✓ verified, ✗ invalid/tampered, ◯ absent.

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use secretariat_core::application::{
    verify_document_layered, LayeredVerifyOutcome, SignatureOutcome, VerifyOutcome,
};
use secretariat_core::infrastructure::identity_store::load_identity;
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
    // Pass the local principal DID so the layered verifier can short-circuit
    // the agent-binding check when the signer IS the principal themselves.
    let local_did = load_identity(&paths.identity_md)
        .ok()
        .flatten()
        .map(|id| id.did);
    let outcome =
        verify_document_layered(&args.file, &resolver, local_did.as_ref())
            .with_context(|| format!("verifying {}", args.file.display()))?;

    if args.json {
        print_json(&outcome);
    } else {
        print_human(&outcome);
    }

    // Exit codes:
    //   0 — both layers are clean (signature Ok or absent + stamp verified)
    //   2 — either layer reports invalid / tampered / unresolvable
    let signature_ok = matches!(
        outcome.signature,
        SignatureOutcome::Ok { .. }
            | SignatureOutcome::OkUnverifiedAgent { .. }
            | SignatureOutcome::None
    );
    let stamp_ok = matches!(outcome.stamp, VerifyOutcome::Verified { .. } | VerifyOutcome::Unsigned);
    if !(signature_ok && stamp_ok) {
        std::process::exit(2);
    }
    Ok(())
}

fn print_human(out: &LayeredVerifyOutcome) {
    // Author signature layer.
    match &out.signature {
        SignatureOutcome::None => {
            println!("◯ signature  none (no $signature block)");
        }
        SignatureOutcome::Ok {
            signer,
            signer_role,
            signed_at,
        } => {
            println!("✓ signature  {signer_role} {signer} at {signed_at}");
        }
        SignatureOutcome::OkUnverifiedAgent { signer, signed_at } => {
            println!(
                "△ signature  agent {signer} at {signed_at} (agent→principal binding not yet verified — Move 1C/Phase C)"
            );
        }
        SignatureOutcome::Tampered {
            claimed_hash,
            computed_hash,
        } => {
            println!(
                "✗ signature  tampered (claimed {claimed_hash}, computed {computed_hash})"
            );
        }
        SignatureOutcome::SignerUnresolvable { signer, cause } => {
            println!("✗ signature  cannot resolve {signer}: {cause}");
        }
        SignatureOutcome::Invalid { signer } => {
            println!("✗ signature  invalid (does not verify for {signer})");
        }
    }
    // Principal stamp layer.
    match &out.stamp {
        VerifyOutcome::Unsigned => {
            println!("◯ stamp      none (no $attestation block)");
        }
        VerifyOutcome::Verified {
            signer,
            stamped_at,
            act,
        } => {
            println!("✓ stamp      {signer} at {stamped_at} (act: {act})");
        }
        VerifyOutcome::Tampered {
            claimed_hash,
            computed_hash,
        } => {
            println!(
                "✗ stamp      tampered (claimed {claimed_hash}, computed {computed_hash})"
            );
        }
        VerifyOutcome::SignerUnresolvable { signer, cause } => {
            println!("✗ stamp      cannot resolve {signer}: {cause}");
        }
        VerifyOutcome::SignatureInvalid { signer } => {
            println!("✗ stamp      invalid (does not verify for {signer})");
        }
    }
}

fn print_json(out: &LayeredVerifyOutcome) {
    use serde_json::json;
    let signature = match &out.signature {
        SignatureOutcome::None => json!({ "outcome": "none" }),
        SignatureOutcome::Ok {
            signer,
            signer_role,
            signed_at,
        } => json!({
            "outcome": "ok",
            "signer": signer.as_str(),
            "signerRole": signer_role.as_str(),
            "signedAt": signed_at,
        }),
        SignatureOutcome::OkUnverifiedAgent { signer, signed_at } => json!({
            "outcome": "okUnverifiedAgent",
            "signer": signer.as_str(),
            "signedAt": signed_at,
        }),
        SignatureOutcome::Tampered {
            claimed_hash,
            computed_hash,
        } => json!({
            "outcome": "tampered",
            "claimedHash": claimed_hash.to_string(),
            "computedHash": computed_hash.to_string(),
        }),
        SignatureOutcome::SignerUnresolvable { signer, cause } => json!({
            "outcome": "signerUnresolvable",
            "signer": signer.as_str(),
            "cause": cause.to_string(),
        }),
        SignatureOutcome::Invalid { signer } => json!({
            "outcome": "invalid",
            "signer": signer.as_str(),
        }),
    };
    let stamp = match &out.stamp {
        VerifyOutcome::Unsigned => json!({ "outcome": "none" }),
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
    let v = json!({
        "signature": signature,
        "stamp": stamp,
    });
    println!("{}", serde_json::to_string_pretty(&v).unwrap());
}
