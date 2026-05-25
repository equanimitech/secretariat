//! Use case: send a stamped envelope to its recipient's relay.
//!
//! Reads a stamped markdown file from the outbox, looks up the recipient's
//! contact, posts the bytes to that contact's relay, and moves the file to
//! the `sent/` subdirectory.
//!
//! Two callers today: the daemon's outbox-drain loop (background) and the
//! `sec stamp` CLI (immediate-send-on-stamp). The MCP `stamp` tool and the
//! Tauri `stamp_draft` command will both eventually wire through here too —
//! keeping a single source of truth for "send a stamped envelope" semantics.

use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use thiserror::Error;

use crate::infrastructure::contact_store::ContactBook;
use crate::infrastructure::markdown::{parse_document, MarkdownError};
use crate::infrastructure::transport::RelayClient;

#[derive(Debug, Error)]
pub enum SendError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("envelope {path} is not valid UTF-8")]
    InvalidUtf8 { path: PathBuf },
    #[error("parsing envelope: {0}")]
    ParseEnvelope(#[from] MarkdownError),
    #[error("envelope frontmatter missing — composer should have written it")]
    EnvelopeMissing,
    #[error("envelope is not stamped yet — principal must approve before sending")]
    NotStamped,
    #[error("no contact for recipient {recipient}")]
    NoContact { recipient: String },
    #[error("contact `{name}` has no relay_endpoint and v0 does not yet do live did:web service-endpoint discovery")]
    NoEndpoint { name: String },
    #[error("relay send: {0}")]
    Relay(String),
}

/// Outcome of a successful send.
#[derive(Debug, Clone)]
pub struct SendOutcome {
    pub recipient_did: String,
    pub relay_endpoint: String,
    pub relay_assigned_id: u64,
    pub moved_to: PathBuf,
}

/// Attempt to send a stamped envelope. Returns the relay-assigned id and
/// the path the file was moved to (`sent_dir/<original-name>`).
///
/// Idempotency: callers that hit this twice for the same file will get
/// `Io` (file not found) on the second pass, since the first pass renames
/// the file out from under itself. That's fine — the daemon's outbox-drain
/// iterates a fresh directory listing each tick.
pub async fn send_stamped_envelope(
    file_path: &Path,
    contacts: &ContactBook,
    key: &SigningKey,
    sent_dir: &Path,
) -> Result<SendOutcome, SendError> {
    let raw = std::fs::read(file_path).map_err(|e| SendError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;
    let raw_str = std::str::from_utf8(&raw).map_err(|_| SendError::InvalidUtf8 {
        path: file_path.to_path_buf(),
    })?;
    let parsed = parse_document(raw_str)?;

    let envelope = parsed.envelope.ok_or(SendError::EnvelopeMissing)?;
    if parsed.stamp.is_none() {
        return Err(SendError::NotStamped);
    }
    // Self-addressed-shortcut removed in Move 3a (substrate-for-themia
    // address collapse, 2026-05-21). Under the new substrate, self-owned
    // channels live under `channels/<slug>/` and never federate — but
    // that routing decision belongs to the daemon's endpoint-resolution
    // chain (Move 5), not to this use case. Until Move 5 lands, an
    // envelope addressed to self will fall through to `NoContact` here,
    // which existing consumers already classify as a warning.
    let recipient_did = &envelope.recipient.owner;

    let contact = contacts
        .find_by_did(recipient_did)
        .ok_or_else(|| SendError::NoContact {
            recipient: recipient_did.as_str().to_string(),
        })?;
    let endpoint = contact
        .relay_endpoint
        .as_ref()
        .ok_or_else(|| SendError::NoEndpoint {
            name: contact.display_name.to_string(),
        })?;

    let client = RelayClient::new(endpoint.as_str(), envelope.from.clone(), key);
    let id = client
        .send(
            &envelope.recipient.owner,
            &envelope.recipient.handle,
            &raw,
            "text/markdown",
        )
        .await
        .map_err(|e| SendError::Relay(e.to_string()))?;

    let file_name = file_path
        .file_name()
        .ok_or_else(|| SendError::Io {
            path: file_path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "no file name"),
        })?;
    let dest = sent_dir.join(file_name);
    std::fs::create_dir_all(sent_dir).map_err(|e| SendError::Io {
        path: sent_dir.to_path_buf(),
        source: e,
    })?;
    std::fs::rename(file_path, &dest).map_err(|e| SendError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;

    Ok(SendOutcome {
        recipient_did: recipient_did.as_str().to_string(),
        relay_endpoint: endpoint.as_str().to_string(),
        relay_assigned_id: id,
        moved_to: dest,
    })
}
