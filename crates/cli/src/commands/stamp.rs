//! `sec stamp` — biometric attestation of a markdown file.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Parser;
use std::path::PathBuf;

use secretariat_core::application::{stamp_document, StampError};
use secretariat_core::domain::StampAct;
use secretariat_core::infrastructure::keys::load_signing_key;
use secretariat_core::ports::SignerError;

use super::biometric;
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
    let paths = key_paths()?;
    let did = load_did(&paths)?;
    let key = load_signing_key(&paths.signing_key)
        .with_context(|| format!("loading {} (run `sec init` first)", paths.signing_key.display()))?;

    let signer = biometric::build_signer(did.clone(), key, args.allow_test_biometrics)?;
    let act: StampAct = args.act.into();

    match stamp_document(&args.file, &signer, act, args.force, Utc::now()) {
        Ok(out) => {
            println!(
                "✓ stamped {} at {} (signer {})",
                out.stamped_path.display(),
                out.stamp.stamped_at,
                out.stamp.signer
            );
            Ok(())
        }
        Err(StampError::AlreadyStamped) => {
            eprintln!("file is already stamped — pass --force to re-stamp");
            std::process::exit(2);
        }
        Err(StampError::Signer(SignerError::BiometricRefused)) => {
            eprintln!("biometric refused or cancelled");
            std::process::exit(3);
        }
        Err(e) => Err(anyhow!(e)),
    }
}

