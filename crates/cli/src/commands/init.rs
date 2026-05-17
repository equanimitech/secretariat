//! `sec init` — one-time setup.
//!
//! Generates an ed25519 signing key, derives the principal's DID, seeds the
//! user-customizable AG template, and creates the inbox/outbox/peers directories.
//!
//! Default DID method: `did:key` (zero hosting). Pass `--did did:web:<host>`
//! to use a domain-anchored identity that survives key rotation.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Parser;
use std::fs;

use secretariat_core::domain::{DidMethod, DisplayName};
use secretariat_core::infrastructure::identity_store::{
    save_identity, PrincipalIdentity,
};
use secretariat_core::infrastructure::keys::{
    generate_keypair, save_signing_key, write_did_document,
};
use secretariat_core::Did;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    /// Pass an explicit `did:web:<host>[:<path>]` to anchor identity in a
    /// domain you control. Omit to derive a `did:key` from the freshly
    /// generated public key (zero hosting).
    #[arg(long)]
    did: Option<String>,

    /// Re-seed the template even if it already exists.
    /// Never overwrites an existing signing key.
    #[arg(long, default_value_t = false)]
    force_seed: bool,
}

const TEMPLATE_DEFAULT: &str = include_str!("../../assets/template_default.md");

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;

    // 1. Refuse if a key already exists.
    if paths.signing_key.exists() {
        return Err(anyhow!(
            "signing key already exists at {} — refusing to overwrite",
            paths.signing_key.display()
        ));
    }

    // 2. Generate the signing key.
    let key = generate_keypair();

    // 3. Resolve the DID. If --did is supplied, validate. Otherwise derive a did:key.
    let did = match args.did {
        None => Did::from_ed25519_public_key(&key.verifying_key().to_bytes()),
        Some(s) => {
            let parsed = Did::parse(&s).map_err(|e| anyhow!("invalid --did: {e}"))?;
            if parsed.method() == DidMethod::Key {
                return Err(anyhow!(
                    "did:key is auto-derived from the new key — omit --did to use it"
                ));
            }
            parsed
        }
    };

    // 4. Persist the signing key (now that we know we'll succeed).
    save_signing_key(&paths.signing_key, &key)
        .with_context(|| format!("writing key to {}", paths.signing_key.display()))?;

    // 5. did:web only: write the hosted document scaffold.
    if did.method() == DidMethod::Web {
        write_did_document(&paths.did_document, &did, &key.verifying_key())
            .with_context(|| format!("writing did.json to {}", paths.did_document.display()))?;
    }

    // 6. Seed AG template.
    seed_file(&paths.template, TEMPLATE_DEFAULT, args.force_seed)?;

    // 7. Persist identity record at `_self/identity.md` (the canonical
    // location for the principal's DID + profile + key metadata).
    let now = Utc::now();
    let display_name = DisplayName::parse("Principal")
        .map_err(|e| anyhow!("default display name invalid: {e}"))?;
    let did_method = match did.method() {
        DidMethod::Key => "did:key",
        DidMethod::Web => "did:web",
    };
    let identity = PrincipalIdentity {
        did: did.clone(),
        did_method: did_method.to_string(),
        display_name,
        full_name: None,
        key_path: "identity/key".to_string(),
        key_type: "ed25519".to_string(),
        key_created_at: now,
        key_rotations: Vec::new(),
        created_at: now,
        body: String::new(),
    };
    save_identity(&paths.identity_md, &identity)
        .with_context(|| format!("writing {}", paths.identity_md.display()))?;

    // 8. Report. (Biometric gate is in-process — no helper binary to install.)
    eprintln!("[sec] initialized at {}", paths.root.display());
    eprintln!("[sec]   did          → {did}");
    eprintln!("[sec]   identity     → {}", paths.identity_md.display());
    eprintln!("[sec]   signing key  → {}", paths.signing_key.display());
    eprintln!("[sec]   template     → {}", paths.template.display());
    match did.method() {
        DidMethod::Key => {
            eprintln!();
            eprintln!("Identity is a did:key — no hosting needed. Recipients verify offline.");
        }
        DidMethod::Web => {
            eprintln!("[sec]   did document → {}", paths.did_document.display());
            if let Some(url) = did.web_document_url() {
                eprintln!();
                eprintln!("Next: host {} at:", paths.did_document.display());
                eprintln!("    {url}");
            }
        }
    }
    Ok(())
}

fn seed_file(path: &std::path::Path, content: &str, overwrite: bool) -> Result<()> {
    if path.exists() && !overwrite {
        eprintln!(
            "[sec] {} already exists, leaving alone (use --force-seed to overwrite)",
            path.display()
        );
        return Ok(());
    }
    fs::write(path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
