//! `sec stamp` — biometric attestation of a markdown file.
//!
//! After a successful stamp, immediately attempts to deliver the envelope
//! to the recipient's relay. The principal's stamp is the "send" intent;
//! decoupling that from delivery (waiting for the next daemon tick) is a
//! T2FM blocker. If immediate-send fails (contact unknown, network error,
//! relay unreachable), the file stays in the outbox and the daemon's next
//! tick retries — so this is a best-effort accelerator, not a new failure
//! surface.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Parser;
use std::path::{Path, PathBuf};

use secretariat_core::application::{
    send_stamped_envelope, stamp_document, SendError, StampError,
};
use secretariat_core::domain::StampAct;
use secretariat_core::infrastructure::contact_store::ContactBook;
use secretariat_core::infrastructure::keys::{load_signing_key, KeyPaths};
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

    /// Stamp only — don't try to deliver immediately. The next daemon tick
    /// will pick it up. Use when you're stamping a file that isn't an
    /// outbox draft (e.g. a standalone markdown attestation).
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

    let signer = biometric::build_signer(did.clone(), key.clone(), args.allow_test_biometrics)?;
    let act: StampAct = args.act.into();

    let outcome = match stamp_document(&args.file, &signer, act, args.force, Utc::now()) {
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

    println!(
        "✓ stamped {} at {} (signer {})",
        outcome.stamped_path.display(),
        outcome.stamp.stamped_at,
        outcome.stamp.signer
    );

    if args.no_send {
        return Ok(());
    }

    // Try immediate delivery. If anything goes wrong (file isn't in an
    // outbox/<recipient>/ directory, contact unknown, network down), fall
    // back silently to the daemon's regular tick — the file stays in the
    // outbox.
    match try_send_now(&outcome.stamped_path, &paths, &key) {
        Ok(Some(out)) => println!(
            "✓ delivered to {} (relay id {})",
            out.relay_endpoint, out.relay_assigned_id
        ),
        Ok(None) => {
            // Not an outbox file — nothing to deliver. Quiet success.
        }
        Err(e) => eprintln!(
            "stamp ok; immediate delivery skipped ({e}). The daemon will retry on its next tick."
        ),
    }

    Ok(())
}

/// Returns `Some(SendOutcome)` if the file was delivered, `None` if it
/// wasn't an outbox draft (e.g. a standalone stamped attestation).
fn try_send_now(
    stamped_path: &Path,
    paths: &KeyPaths,
    key: &ed25519_dalek::SigningKey,
) -> Result<Option<secretariat_core::application::SendOutcome>> {
    // Only auto-deliver when the file lives directly inside any
    // `outbox/` directory. The v0.3 substrate has one outbox per
    // queue (`<root>/<alias>/<namespace>/<segments>/outbox/`); any
    // file whose immediate parent is named `outbox` qualifies. Files
    // stamped elsewhere (standalone attestations, draft repos) won't
    // match and are quietly skipped.
    let parent = match stamped_path.parent() {
        Some(p) => p,
        None => return Ok(None),
    };
    let inside_outbox = parent.file_name().and_then(|n| n.to_str()) == Some("outbox");
    if !inside_outbox {
        return Ok(None);
    }
    let sent_dir = parent.join("sent");

    let contacts = ContactBook::load(&paths.contacts).context("loading contacts")?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime for immediate send")?;

    match runtime.block_on(send_stamped_envelope(stamped_path, &contacts, key, &sent_dir)) {
        Ok(out) => Ok(Some(out)),
        Err(SendError::NotStamped) => {
            // Shouldn't happen — we just stamped it. But handle gracefully.
            Ok(None)
        }
        Err(e) => Err(anyhow!(e)),
    }
}
