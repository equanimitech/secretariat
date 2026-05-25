//! `sec stamp` — biometric attestation of a markdown file.
//!
//! After a successful stamp, the file is atomically renamed from the
//! per-queue `_drafts/` dir into the canonical `envelopes/YYYY/MM/DD/`
//! day-shard, then immediate delivery is attempted via the recipient's
//! relay. The principal's stamp is the "send" intent; decoupling that
//! from delivery (waiting for the next daemon tick) is a T2FM blocker.
//! If immediate-send fails (contact unknown, network error, relay
//! unreachable), the stamped file stays in `envelopes/` and the
//! daemon's next tick retries — best-effort accelerator, not a new
//! failure surface.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Parser;
use std::path::{Path, PathBuf};

use secretariat_core::application::{
    send_stamped_envelope, stamp_document, SendError, StampError,
};
use secretariat_core::domain::StampAct;
use secretariat_core::infrastructure::biometric::build_signer;
use secretariat_core::infrastructure::contact_store::ContactBook;
use secretariat_core::infrastructure::keys::{load_signing_key, KeyPaths};
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

    /// Stamp only — don't try to deliver immediately. The next daemon tick
    /// will pick it up. Use when you're stamping a file that isn't a
    /// draft envelope (e.g. a standalone markdown attestation).
    #[arg(long, default_value_t = false)]
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

    if args.no_send {
        return Ok(());
    }

    // Try immediate delivery. If anything goes wrong (file isn't in
    // an `envelopes/` tree, contact unknown, network down), fall back
    // silently to the daemon's regular tick — the file stays put.
    match try_send_now(&stamped_path, &paths, &key) {
        Ok(Some(out)) => println!(
            "✓ delivered to {} (relay id {})",
            out.relay_endpoint, out.relay_assigned_id
        ),
        Ok(None) => {
            // Not a deliverable envelope (e.g. a standalone attestation).
        }
        Err(e) => eprintln!(
            "stamp ok; immediate delivery skipped ({e}). The daemon will retry on its next tick."
        ),
    }

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

/// Returns `Some(SendOutcome)` if the file was delivered, `None` if it
/// wasn't a deliverable envelope (e.g. a standalone stamped attestation).
fn try_send_now(
    stamped_path: &Path,
    paths: &KeyPaths,
    key: &ed25519_dalek::SigningKey,
) -> Result<Option<secretariat_core::application::SendOutcome>> {
    // Only auto-deliver when the file lives inside an `envelopes/`
    // tree under some queue dir — i.e. an ancestor named `envelopes`.
    // The post-stamp rename in `promote_draft_to_envelope` puts the
    // file at `<queue>/envelopes/YYYY/MM/DD/<file>.md`. Standalone
    // attestations stamped elsewhere quietly skip.
    let mut envelopes_anchor: Option<&Path> = None;
    for ancestor in stamped_path.ancestors() {
        if ancestor.file_name().and_then(|n| n.to_str()) == Some("envelopes") {
            envelopes_anchor = Some(ancestor);
            break;
        }
    }
    let envelopes_anchor = match envelopes_anchor {
        Some(p) => p,
        None => return Ok(None),
    };
    let queue_dir = match envelopes_anchor.parent() {
        Some(p) => p,
        None => return Ok(None),
    };

    // Mirror the day-shard relative path under `sent/`.
    let sent_root = queue_dir.join("sent");
    let day_shard = stamped_path
        .parent()
        .and_then(|parent| parent.strip_prefix(envelopes_anchor).ok())
        .map(|rel| sent_root.join(rel))
        .unwrap_or(sent_root);

    let contacts = ContactBook::load(&paths.contacts).context("loading contacts")?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime for immediate send")?;

    match runtime.block_on(send_stamped_envelope(stamped_path, &contacts, key, &day_shard)) {
        Ok(out) => Ok(Some(out)),
        Err(SendError::NotStamped) => {
            // Shouldn't happen — we just stamped it. But handle gracefully.
            Ok(None)
        }
        // SelfAddressed dropped in Move 3a; self-owned-channel routing
        // moves to the daemon in Move 5. Until then, self-addressed
        // stamps surface as NoContact errors and bubble up.
        Err(e) => Err(anyhow!(e)),
    }
}
