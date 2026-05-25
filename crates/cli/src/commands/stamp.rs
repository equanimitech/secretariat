//! `sec stamp` — biometric attestation of a markdown file.
//!
//! Stamping embeds the `$attestation` block in the envelope's
//! frontmatter in place. Post-Move 4 (substrate-for-themia) there
//! is no path rename — every envelope already lives at
//! `<queue>/envelopes/YYYY/MM/DD/<rkey>.md`. Federation runs in the
//! daemon: it picks up envelopes whose frontmatter lacks
//! `delivered:` and writes the field on success. `sec stamp` itself
//! does not attempt immediate delivery.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Parser;
use std::path::PathBuf;

use secretariat_core::application::{stamp_document, StampError};
use secretariat_core::domain::StampAct;
use secretariat_core::infrastructure::biometric::build_signer;
use secretariat_core::infrastructure::keys::load_signing_key;
use secretariat_core::ports::SignerError;

use super::paths::{key_paths, load_did};

#[derive(Parser, Debug)]
pub struct Args {
    /// File to stamp.
    file: PathBuf,

    /// Stamp act. MVP supports `attest`.
    #[arg(long, value_enum, default_value_t = ActArg::Attest)]
    act: ActArg,

    /// Re-stamp even if a stamp is already present.
    #[arg(long, default_value_t = false)]
    force: bool,

    /// Required when `SECRETARIAT_BIOMETRIC=always_allow|always_deny` is set
    /// in a release build. Refuses to honor those test gates otherwise.
    #[arg(long, default_value_t = false)]
    allow_test_biometrics: bool,

    /// Kept for backward compatibility. Federation now runs in the
    /// daemon, so stamp never tries to send synchronously — this flag
    /// is a no-op.
    #[arg(long, default_value_t = false, hide = true)]
    no_send: bool,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum ActArg {
    Attest,
    Defer,
    Vouch,
    Dispute,
    Redirect,
}

impl From<ActArg> for StampAct {
    fn from(v: ActArg) -> Self {
        match v {
            ActArg::Attest => StampAct::Attest,
            ActArg::Defer => StampAct::Defer,
            ActArg::Vouch => StampAct::Vouch,
            ActArg::Dispute => StampAct::Dispute,
            ActArg::Redirect => StampAct::Redirect,
        }
    }
}

pub fn run(args: Args) -> Result<()> {
    let _ = args.no_send; // accepted for backward compat, no-op now
    let paths = key_paths()?;
    let did = load_did(&paths)?;
    let key = load_signing_key(&paths.signing_key).with_context(|| {
        format!(
            "loading {} (run `sec init` first)",
            paths.signing_key.display()
        )
    })?;

    let signer = build_signer(did.clone(), key.clone(), args.allow_test_biometrics)?;
    let act: StampAct = args.act.into();

    let now = Utc::now();
    let outcome = match stamp_document(&args.file, &signer, act, args.force, now) {
        Ok(out) => out,
        Err(StampError::AlreadyStamped) => {
            eprintln!("file is already stamped — pass --force to re-stamp");
            std::process::exit(2);
        }
        Err(StampError::Signer(SignerError::BiometricRefused)) => {
            eprintln!("biometric refused or cancelled");
            std::process::exit(3);
        }
        Err(e) => return Err(anyhow!(e)),
    };

    // Stamp embeds the `$attestation` block in place; no path
    // rename. Federation is the daemon's job — it watches the
    // `envelopes/` tree for files lacking `delivered:` frontmatter
    // and writes the field on successful relay push.
    println!(
        "✓ stamped {} at {} (signer {})",
        outcome.stamped_path.display(),
        outcome.stamp.stamped_at,
        outcome.stamp.signer
    );

    Ok(())
}
