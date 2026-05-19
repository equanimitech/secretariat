//! HTTP client for the Secretariat relay (`crates/relay`).
//!
//! Concretely: register once, authenticate (challenge → signed answer →
//! short-lived bearer token), POST envelopes addressed to other DIDs, GET
//! envelopes addressed to us. Cursor-paginated polling.
//!
//! ## State model
//!
//! [`RelayState`] tracks per-relay state across daemon restarts:
//! - `endpoint` — the relay base URL (matches a contact's `relay_endpoint`
//!   or the principal's own self-hosted relay)
//! - `registered` — whether we've completed the one-time registration
//! - `token` + `token_expires_at` — current bearer token (re-authenticate
//!   when expired)
//! - `cursor` — highest envelope id we've successfully ingested; `?after=<cursor>`
//!   on next poll
//!
//! State is persisted at `~/.secretariat/relay-state.json` (atomic write,
//! mode 0600 on Unix).

use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::codec::encode_ed25519_multibase;
use crate::domain::{Did, QueueHandle};

const REGISTER_DOMAIN: &[u8] = b"secretariat-relay-register:v0:";
const AUTH_DOMAIN: &[u8] = b"secretariat-relay-auth:v0:";
const STATE_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum RelayClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("relay returned status {status}: {body}")]
    BadStatus { status: u16, body: String },
    #[error("relay response did not match expected schema: {0}")]
    BadResponse(String),
    #[error("base64 decode failed: {0}")]
    Base64(#[from] base64::DecodeError),
}

#[derive(Debug, Error)]
pub enum RelayStateError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("relay state json malformed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("relay state has unsupported version {0} (this build understands {STATE_VERSION})")]
    UnsupportedVersion(u32),
}

// ---------------------------------------------------------------------------
// Persistent state
// ---------------------------------------------------------------------------

/// Per-relay session + cursor state. One entry per relay we talk to.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelayEntry {
    pub endpoint: String,
    #[serde(default)]
    pub registered: bool,
    #[serde(default)]
    pub cursor: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StateFile {
    version: u32,
    relays: Vec<RelayEntry>,
}

impl Default for StateFile {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            relays: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RelayState {
    relays: Vec<RelayEntry>,
}

impl RelayState {
    pub fn load(path: &Path) -> Result<Self, RelayStateError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path).map_err(|e| RelayStateError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let parsed: StateFile = serde_json::from_str(&raw)?;
        if parsed.version != STATE_VERSION {
            return Err(RelayStateError::UnsupportedVersion(parsed.version));
        }
        Ok(Self {
            relays: parsed.relays,
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), RelayStateError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| RelayStateError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let snapshot = StateFile {
            version: STATE_VERSION,
            relays: self.relays.clone(),
        };
        let pretty = serde_json::to_string_pretty(&snapshot)?;

        let parent = path.parent().unwrap_or(Path::new("."));
        let mut tmp = NamedTempFile::new_in(parent).map_err(|e| RelayStateError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
        use std::io::Write as _;
        tmp.write_all(pretty.as_bytes())
            .and_then(|_| tmp.write_all(b"\n"))
            .map_err(|e| RelayStateError::Io {
                path: tmp.path().to_path_buf(),
                source: e,
            })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(tmp.path(), perms).map_err(|e| RelayStateError::Io {
                path: tmp.path().to_path_buf(),
                source: e,
            })?;
        }

        tmp.persist(path).map_err(|e| RelayStateError::Io {
            path: path.to_path_buf(),
            source: e.error,
        })?;
        Ok(())
    }

    pub fn entry_mut(&mut self, endpoint: &str) -> &mut RelayEntry {
        if let Some(idx) = self.relays.iter().position(|r| r.endpoint == endpoint) {
            &mut self.relays[idx]
        } else {
            self.relays.push(RelayEntry {
                endpoint: endpoint.to_string(),
                ..Default::default()
            });
            self.relays.last_mut().unwrap()
        }
    }

    pub fn entry(&self, endpoint: &str) -> Option<&RelayEntry> {
        self.relays.iter().find(|r| r.endpoint == endpoint)
    }

    pub fn iter(&self) -> impl Iterator<Item = &RelayEntry> {
        self.relays.iter()
    }
}

// ---------------------------------------------------------------------------
// Inbound envelopes
// ---------------------------------------------------------------------------

/// One envelope pulled from the relay. The body is the raw bytes the sender
/// POSTed (typically a markdown file with frontmatter + an encrypted body).
#[derive(Debug, Clone)]
pub struct RelayInbound {
    pub id: u64,
    pub queued_at: DateTime<Utc>,
    pub sender_did: Option<Did>,
    pub body: Vec<u8>,
    pub content_type: String,
}

#[derive(Deserialize)]
struct PollResponseWire {
    envelopes: Vec<InboundWire>,
}

#[derive(Deserialize)]
struct InboundWire {
    id: u64,
    queued_at: DateTime<Utc>,
    #[serde(default)]
    sender_did: Option<Did>,
    /// base64-encoded bytes
    body: String,
    content_type: String,
}

// ---------------------------------------------------------------------------
// RelayClient
// ---------------------------------------------------------------------------

/// HTTP client for one relay endpoint.
pub struct RelayClient<'a> {
    pub endpoint: String,
    pub did: Did,
    pub signing_key: &'a SigningKey,
    http: Client,
}

impl<'a> RelayClient<'a> {
    pub fn new(endpoint: impl Into<String>, did: Did, signing_key: &'a SigningKey) -> Self {
        let endpoint = endpoint.into();
        let endpoint = endpoint.trim_end_matches('/').to_string();
        Self {
            endpoint,
            did,
            signing_key,
            http: Client::new(),
        }
    }

    /// One-time registration. Idempotent on the server side (returns 409 if
    /// already registered — we treat that as success).
    pub async fn register(&self) -> Result<(), RelayClientError> {
        let pubkey_bytes = self.signing_key.verifying_key().to_bytes();
        let pubkey_mb = encode_ed25519_multibase(&pubkey_bytes);

        let mut to_sign = REGISTER_DOMAIN.to_vec();
        to_sign.extend_from_slice(self.did.as_str().as_bytes());
        to_sign.extend_from_slice(&pubkey_bytes);
        let sig = self.signing_key.sign(&to_sign);
        let sig_str = format!("ed25519:{}", B64.encode(sig.to_bytes()));

        let r = self
            .http
            .post(format!("{}/v0/register", self.endpoint))
            .json(&serde_json::json!({
                "did": self.did.as_str(),
                "pubkey_multibase": pubkey_mb,
                "signature": sig_str,
            }))
            .send()
            .await?;

        let status = r.status();
        if status.is_success() || status == reqwest::StatusCode::CONFLICT {
            // 201 = newly registered; 409 = already registered. Both are OK.
            return Ok(());
        }
        let body = r.text().await.unwrap_or_default();
        Err(RelayClientError::BadStatus {
            status: status.as_u16(),
            body,
        })
    }

    /// Challenge → answer → bearer token. Caller stores token + expiry in
    /// [`RelayState`].
    pub async fn authenticate(&self) -> Result<(String, DateTime<Utc>), RelayClientError> {
        let challenge: serde_json::Value = self
            .http
            .post(format!("{}/v0/auth/challenge", self.endpoint))
            .json(&serde_json::json!({ "did": self.did.as_str() }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let nonce = challenge["nonce"]
            .as_str()
            .ok_or_else(|| RelayClientError::BadResponse("challenge missing nonce".into()))?
            .to_string();

        let mut to_sign = AUTH_DOMAIN.to_vec();
        to_sign.extend_from_slice(nonce.as_bytes());
        let sig = self.signing_key.sign(&to_sign);
        let sig_str = format!("ed25519:{}", B64.encode(sig.to_bytes()));

        let answer: serde_json::Value = self
            .http
            .post(format!("{}/v0/auth/answer", self.endpoint))
            .json(&serde_json::json!({
                "did": self.did.as_str(),
                "nonce": nonce,
                "signature": sig_str,
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let token = answer["token"]
            .as_str()
            .ok_or_else(|| RelayClientError::BadResponse("answer missing token".into()))?
            .to_string();
        let expires_at_str = answer["expires_at"]
            .as_str()
            .ok_or_else(|| RelayClientError::BadResponse("answer missing expires_at".into()))?;
        let expires_at = DateTime::parse_from_rfc3339(expires_at_str)
            .map_err(|e| RelayClientError::BadResponse(format!("bad expires_at: {e}")))?
            .with_timezone(&Utc);

        Ok((token, expires_at))
    }

    // ---------------------------------------------------------------------
    // Channel queue (v0.8) — `(owner, handle)` index axis.
    // ---------------------------------------------------------------------

    /// POST an envelope to an `(owner, handle)` queue. Generalizes
    /// `send` (which is the two-party case with handle `inbox:default`,
    /// addressed through the legacy `/v0/inbox/:did` route). Body is opaque
    /// bytes; relay queues by `(owner, handle)` and assigns the per-channel seq.
    ///
    /// Handle is percent-encoded into the path segment (colons → `%3A`); the
    /// `reqwest::Url` builder handles this for path segments automatically.
    pub async fn send(
        &self,
        owner: &Did,
        handle: &QueueHandle,
        body: &[u8],
        content_type: &str,
    ) -> Result<u64, RelayClientError> {
        let url = format!(
            "{}/v0/queue/{}/{}",
            self.endpoint,
            owner.as_str(),
            encode_handle(handle),
        );
        let r = self
            .http
            .post(url)
            .header("content-type", content_type)
            .header("x-sender-did", self.did.as_str())
            .body(body.to_vec())
            .send()
            .await?;
        let status = r.status();
        if !status.is_success() {
            let body = r.text().await.unwrap_or_default();
            return Err(RelayClientError::BadStatus {
                status: status.as_u16(),
                body,
            });
        }
        let parsed: serde_json::Value = r.json().await?;
        let id = parsed["id"]
            .as_u64()
            .ok_or_else(|| RelayClientError::BadResponse("post response missing id".into()))?;
        Ok(id)
    }

    /// Pull entries with `id > after` from the `(owner, handle)` channel
    /// queue. Caller must hold a valid bearer token (we send our own DID's).
    pub async fn poll(
        &self,
        owner: &Did,
        handle: &QueueHandle,
        token: &str,
        after: u64,
    ) -> Result<Vec<RelayInbound>, RelayClientError> {
        let url = format!(
            "{}/v0/queue/{}/{}",
            self.endpoint,
            owner.as_str(),
            encode_handle(handle),
        );
        let r = self
            .http
            .get(url)
            .query(&[("after", after)])
            .header("authorization", format!("Bearer {token}"))
            .send()
            .await?;
        let status = r.status();
        if !status.is_success() {
            let body = r.text().await.unwrap_or_default();
            return Err(RelayClientError::BadStatus {
                status: status.as_u16(),
                body,
            });
        }
        let parsed: PollResponseWire = r.json().await?;
        parsed
            .envelopes
            .into_iter()
            .map(|w| {
                let body = B64.decode(w.body)?;
                Ok(RelayInbound {
                    id: w.id,
                    queued_at: w.queued_at,
                    sender_did: w.sender_did,
                    body,
                    content_type: w.content_type,
                })
            })
            .collect()
    }

    /// Pull every claimed invite where this principal is the inviter.
    /// Used by the daemon to discover claim events and auto-add the
    /// claimer as a contact (bidirectional contact-add — the defining
    /// behavior of a correspondence invite).
    ///
    /// Idempotent on the relay side: returns the same list across calls
    /// until the invite is pruned. The daemon dedupes against its local
    /// contact book.
    pub async fn claimed_invites(
        &self,
        token: &str,
    ) -> Result<Vec<ClaimedInviteWire>, RelayClientError> {
        let r = self
            .http
            .get(format!("{}/v0/invites/claimed", self.endpoint))
            .header("authorization", format!("Bearer {token}"))
            .send()
            .await?;
        let status = r.status();
        if !status.is_success() {
            let body = r.text().await.unwrap_or_default();
            return Err(RelayClientError::BadStatus {
                status: status.as_u16(),
                body,
            });
        }
        let parsed: ClaimedListWire = r.json().await?;
        Ok(parsed.invites)
    }
}

/// Percent-encode a [`QueueHandle`] for a URL path segment. Grammar
/// (`[a-z0-9_-:]`) means only `:` needs encoding for path-safety —
/// no need to pull in `percent-encoding` for one char.
fn encode_handle(h: &QueueHandle) -> String {
    h.as_str().replace(':', "%3A")
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct ClaimedInviteWire {
    pub token: String,
    pub claimant_did: String,
    pub claimed_at: String,
    #[serde(default)]
    pub purpose: Option<String>,
}

#[derive(serde::Deserialize)]
struct ClaimedListWire {
    invites: Vec<ClaimedInviteWire>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn relay_state_load_missing_returns_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("relay-state.json");
        let state = RelayState::load(&path).unwrap();
        assert_eq!(state.iter().count(), 0);
    }

    #[test]
    fn relay_state_save_then_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("relay-state.json");

        let mut state = RelayState::default();
        let entry = state.entry_mut("wss://relay.rafa.equanimi.tech");
        entry.registered = true;
        entry.cursor = 42;
        entry.token = Some("abc".to_string());
        state.save(&path).unwrap();

        let reloaded = RelayState::load(&path).unwrap();
        let e = reloaded.entry("wss://relay.rafa.equanimi.tech").unwrap();
        assert!(e.registered);
        assert_eq!(e.cursor, 42);
        assert_eq!(e.token.as_deref(), Some("abc"));
    }

    #[test]
    fn relay_state_save_writes_0600() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("relay-state.json");
        let mut state = RelayState::default();
        state.entry_mut("wss://relay.example.com").registered = true;
        state.save(&path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn relay_state_entry_mut_creates_and_updates() {
        let mut state = RelayState::default();
        {
            let e = state.entry_mut("wss://x");
            e.cursor = 1;
        }
        {
            let e = state.entry_mut("wss://x");
            assert_eq!(e.cursor, 1);
            e.cursor = 2;
        }
        let e = state.entry("wss://x").unwrap();
        assert_eq!(e.cursor, 2);
        assert_eq!(state.iter().count(), 1);
    }

    #[test]
    fn relay_state_unsupported_version_errors() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("relay-state.json");
        std::fs::write(&path, r#"{"version": 999, "relays": []}"#).unwrap();
        assert!(matches!(
            RelayState::load(&path),
            Err(RelayStateError::UnsupportedVersion(999))
        ));
    }
}
