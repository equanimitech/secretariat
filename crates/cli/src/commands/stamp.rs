//! `sec stamp` — biometric attestation of a markdown file.
//!
//! After a successful stamp, the file is atomically renamed from the
//! per-queue `_drafts/` dir into the canonical `envelopes/YYYY/MM/DD/`
//! day-shard. That rename IS the wire-send signal for the daemon's
//! watcher — federation runs in the daemon (substrate-for-themia,
//! Move 5). `sec stamp` itself no longer attempts immediate delivery.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Parser;
use std::path::{Path, PathBuf};

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
    let key = load_signing_key(&paths.signing_key)
        .with_context(|| format!("loading {} (run `sec init` first)", paths.signing_key.display()))?;

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

    // If the file is a draft (lives under `_drafts/`), atomically
    // rename it into the canonical day-sharded `envelopes/` tree. The
    // rename is the wire-send signal for the daemon's watcher.
    let stamped_path = promote_draft_to_envelope(&outcome.stamped_path, now)?;

    println!(
        "✓ stamped {} at {} (signer {})",
        stamped_path.display(),
        outcome.stamp.stamped_at,
        outcome.stamp.signer
    );

    Ok(())
}

/// Promote a stamped draft from `<queue>/_drafts/<file>` into the
/// canonical `<queue>/envelopes/YYYY/MM/DD/<file>` day-shard. The
/// `<file>` keeps its original filename so the principal can correlate
/// pre/post-stamp paths if needed. Atomic on a single filesystem
/// (rename is guaranteed atomic by POSIX); no-op (returns the input
/// path unchanged) when the file isn't inside `_drafts/`.
fn promote_draft_to_envelope(stamped: &Path, now: chrono::DateTime<Utc>) -> Result<PathBuf> {
    let parent = match stamped.parent() {
        Some(p) => p,
        None => return Ok(stamped.to_path_buf()),
    };
    if parent.file_name().and_then(|n| n.to_str()) != Some("_drafts") {
        return Ok(stamped.to_path_buf());
    }
    let queue_dir = match parent.parent() {
        Some(p) => p,
        None => return Ok(stamped.to_path_buf()),
    };
    let day_shard = queue_dir
        .join("envelopes")
        .join(now.format("%Y/%m/%d").to_string());
    std::fs::create_dir_all(&day_shard)
        .with_context(|| format!("creating {}", day_shard.display()))?;
    let file_name = stamped
        .file_name()
        .ok_or_else(|| anyhow!("stamped path has no filename: {}", stamped.display()))?;
    let dest = day_shard.join(file_name);
    std::fs::rename(stamped, &dest).with_context(|| {
        format!(
            "promoting draft {} -> {}",
            stamped.display(),
            dest.display()
        )
    })?;
    Ok(dest)
}
