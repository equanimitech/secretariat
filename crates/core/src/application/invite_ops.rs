//! Invite primitive — create / view / claim against a Secretariat relay.
//!
//! Three orchestrations that wrap the relay's `/v0/invite/...` endpoints.
//! All HTTP error handling lives here so the CLI and MCP server share a
//! single error surface.
//!
//! ## Wire spec (mirror of `crates/relay/src/routes/invite.rs`)
//!
//! - **Create.** Inviter signs `b"secretariat-relay-invite-create:v0:" ||
//!   inviter_did_bytes || b":" || expires_at_bytes || b":" || purpose_bytes`,
//!   POSTs to `/v0/invite`. Receives a `claim_url` referencing the token.
//! - **View.** Anyone GETs `/v0/invite/<token>` to preview who's inviting +
//!   when it expires. No auth.
//! - **Claim.** Claimant signs `b"secretariat-relay-invite-claim:v0:" ||
//!   token || b":" || claimant_did_bytes || b":" || pubkey_bytes`, POSTs to
//!   `/v0/invite/<token>/claim`. The relay auto-registers the claimant if
//!   not yet registered (single-round-trip onboarding).

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signer as _, SigningKey};
use reqwest::blocking::Client;
use serde::Deserialize;
use thiserror::Error;

use crate::codec::encode_ed25519_multibase;
use crate::Did;

/// Domain-separation tags must match `relay::routes::invite`.
const CREATE_DOMAIN: &[u8] = b"secretariat-relay-invite-create:v0:";
const CLAIM_DOMAIN: &[u8] = b"secretariat-relay-invite-claim:v0:";

/// Default invite TTL when caller doesn't pin one. Mirrors the relay's
/// `DEFAULT_TTL_HOURS`.
pub const DEFAULT_INVITE_TTL_HOURS: i64 = 168;

#[derive(Debug, Error)]
pub enum InviteError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("relay returned status {status}: {body}")]
    BadStatus { status: u16, body: String },
    #[error("relay response did not match expected schema: {0}")]
    BadResponse(String),
    #[error("invite URL malformed: {0}")]
    BadUrl(String),
    #[error("invalid DID in response: {0}")]
    InvalidDid(#[from] crate::domain::DidParseError),
}

#[derive(Debug, Clone)]
pub struct InviteCreated {
    pub token: String,
    pub claim_url: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct InviteView {
    pub inviter_did: Did,
    pub expires_at: DateTime<Utc>,
    pub purpose: Option<String>,
    pub claimed_by: Option<Did>,
    pub install_url: String,
}

#[derive(Debug, Clone)]
pub struct InviteClaimed {
    pub inviter_did: Did,
    pub claimant_did: Did,
    pub claimed_at: DateTime<Utc>,
    pub registered: bool,
}

// ---------------------------------------------------------------------------
// create_invite
// ---------------------------------------------------------------------------

/// Create a one-shot invite at the relay. Inviter must already be a
/// registered tenant of `endpoint`.
pub fn create_invite(
    endpoint: &str,
    inviter_did: &Did,
    inviter_signing_key: &SigningKey,
    purpose: Option<&str>,
    ttl_hours: Option<i64>,
) -> Result<InviteCreated, InviteError> {
    let ttl = ttl_hours.unwrap_or(DEFAULT_INVITE_TTL_HOURS);
    let now = Utc::now();
    let expires_at = now + Duration::hours(ttl);
    let expires_at_str = expires_at.to_rfc3339();

    let mut to_sign = CREATE_DOMAIN.to_vec();
    to_sign.extend_from_slice(inviter_did.as_str().as_bytes());
    to_sign.extend_from_slice(b":");
    to_sign.extend_from_slice(expires_at_str.as_bytes());
    to_sign.extend_from_slice(b":");
    if let Some(p) = purpose {
        to_sign.extend_from_slice(p.as_bytes());
    }
    let sig = inviter_signing_key.sign(&to_sign);
    let sig_str = format!("ed25519:{}", B64.encode(sig.to_bytes()));

    let body = serde_json::json!({
        "inviter_did": inviter_did.as_str(),
        "expires_at": expires_at_str,
        "purpose": purpose,
        "signature": sig_str,
    });

    let endpoint_trimmed = trim_endpoint(endpoint);
    let url = format!("{endpoint_trimmed}/v0/invite");
    let resp = Client::new().post(url).json(&body).send()?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(InviteError::BadStatus {
            status: status.as_u16(),
            body,
        });
    }
    let parsed: CreateRespWire = resp.json()?;
    let claim_url = format!("{endpoint_trimmed}/v0/invite/{}", parsed.token);
    Ok(InviteCreated {
        token: parsed.token,
        claim_url,
        expires_at: DateTime::parse_from_rfc3339(&parsed.expires_at)
            .map_err(|e| InviteError::BadResponse(format!("expires_at: {e}")))?
            .with_timezone(&Utc),
    })
}

// ---------------------------------------------------------------------------
// view_invite
// ---------------------------------------------------------------------------

/// Fetch invite preview. Public endpoint, no auth required.
/// `claim_url` is the URL the inviter shared (e.g.
/// `https://secretariat.equanimi.tech/v0/invite/<token>`).
pub fn view_invite(claim_url: &str) -> Result<InviteView, InviteError> {
    let resp = Client::new().get(claim_url).send()?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(InviteError::BadStatus {
            status: status.as_u16(),
            body,
        });
    }
    let parsed: ViewRespWire = resp.json()?;
    Ok(InviteView {
        inviter_did: Did::parse(&parsed.inviter_did)?,
        expires_at: DateTime::parse_from_rfc3339(&parsed.expires_at)
            .map_err(|e| InviteError::BadResponse(format!("expires_at: {e}")))?
            .with_timezone(&Utc),
        purpose: parsed.purpose,
        claimed_by: match parsed.claimed_by {
            Some(s) => Some(Did::parse(&s)?),
            None => None,
        },
        install_url: parsed.install_url,
    })
}

// ---------------------------------------------------------------------------
// claim_invite
// ---------------------------------------------------------------------------

/// Claim an invite. The relay auto-registers the claimant during this call
/// if they aren't already a tenant — single round trip for first-time setup.
pub fn claim_invite(
    claim_url: &str,
    claimant_did: &Did,
    claimant_signing_key: &SigningKey,
) -> Result<InviteClaimed, InviteError> {
    let token = extract_token(claim_url)?;
    let pubkey_bytes = claimant_signing_key.verifying_key().to_bytes();
    let pubkey_mb = encode_ed25519_multibase(&pubkey_bytes);

    let mut to_sign = CLAIM_DOMAIN.to_vec();
    to_sign.extend_from_slice(token.as_bytes());
    to_sign.extend_from_slice(b":");
    to_sign.extend_from_slice(claimant_did.as_str().as_bytes());
    to_sign.extend_from_slice(b":");
    to_sign.extend_from_slice(&pubkey_bytes);
    let sig = claimant_signing_key.sign(&to_sign);
    let sig_str = format!("ed25519:{}", B64.encode(sig.to_bytes()));

    let body = serde_json::json!({
        "claimant_did": claimant_did.as_str(),
        "claimant_pubkey_multibase": pubkey_mb,
        "signature": sig_str,
    });

    let url = format!("{}/claim", claim_url.trim_end_matches('/'));
    let resp = Client::new().post(url).json(&body).send()?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(InviteError::BadStatus {
            status: status.as_u16(),
            body,
        });
    }
    let parsed: ClaimRespWire = resp.json()?;
    Ok(InviteClaimed {
        inviter_did: Did::parse(&parsed.inviter_did)?,
        claimant_did: Did::parse(&parsed.claimant_did)?,
        claimed_at: DateTime::parse_from_rfc3339(&parsed.claimed_at)
            .map_err(|e| InviteError::BadResponse(format!("claimed_at: {e}")))?
            .with_timezone(&Utc),
        registered: parsed.registered,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn trim_endpoint(s: &str) -> &str {
    s.trim_end_matches('/')
}

/// Extract `<token>` from `https://relay/v0/invite/<token>`.
fn extract_token(claim_url: &str) -> Result<String, InviteError> {
    let trimmed = claim_url.trim_end_matches('/');
    let idx = trimmed
        .rfind("/v0/invite/")
        .ok_or_else(|| InviteError::BadUrl(claim_url.to_string()))?;
    let token = &trimmed[idx + "/v0/invite/".len()..];
    if token.is_empty() || token.contains('/') {
        return Err(InviteError::BadUrl(claim_url.to_string()));
    }
    Ok(token.to_string())
}

#[derive(Deserialize)]
struct CreateRespWire {
    token: String,
    expires_at: String,
}

#[derive(Deserialize)]
struct ViewRespWire {
    inviter_did: String,
    expires_at: String,
    #[serde(default)]
    purpose: Option<String>,
    #[serde(default)]
    claimed_by: Option<String>,
    install_url: String,
}

#[derive(Deserialize)]
struct ClaimRespWire {
    inviter_did: String,
    claimant_did: String,
    claimed_at: String,
    registered: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_token_from_canonical_url() {
        let url = "https://secretariat.equanimi.tech/v0/invite/abc123";
        assert_eq!(extract_token(url).unwrap(), "abc123");
    }

    #[test]
    fn extract_token_rejects_missing_segment() {
        assert!(extract_token("https://example.com/something").is_err());
    }

    #[test]
    fn extract_token_rejects_trailing_path() {
        assert!(
            extract_token("https://relay/v0/invite/abc/extra").is_err(),
            "tokens must not contain `/`"
        );
    }
}
