//! `POST /v0/invite`               — create a one-shot invite token (inviter signs)
//! `GET  /v0/invite/:token`        — public preview (no auth)
//! `POST /v0/invite/:token/claim`  — claimant signs + (optionally) self-registers
//!
//! Invites are additive UX, not gatekeeping. Open registration remains the
//! default path. Invites collapse Marcelo's setup from "init + register +
//! contact-add + tell-Rafa-his-DID" to "click URL + run claim".

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature as DalekSig, Verifier, VerifyingKey};
use secretariat_core::codec::decode_ed25519_multibase;
use secretariat_core::Did;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use crate::state::{AppState, Invite};

const CREATE_DOMAIN: &[u8] = b"secretariat-relay-invite-create:v0:";
const CLAIM_DOMAIN: &[u8] = b"secretariat-relay-invite-claim:v0:";

const DEFAULT_TTL_HOURS: i64 = 168; // 7 days
const MAX_TTL_HOURS: i64 = 720; // 30 days

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreateRequest {
    pub inviter_did: String,
    /// ISO-8601 expiry timestamp. Server clamps to `<= now + MAX_TTL_HOURS`.
    pub expires_at: String,
    #[serde(default)]
    pub purpose: Option<String>,
    /// `ed25519:<base64>` over `CREATE_DOMAIN || inviter_did_bytes || expires_at_bytes || purpose_bytes`.
    pub signature: String,
}

#[derive(Serialize)]
pub struct CreateResponse {
    pub token: String,
    pub expires_at: String,
}

#[derive(Serialize)]
pub struct ViewResponse {
    pub inviter_did: String,
    pub expires_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_by: Option<String>,
    pub install_url: String,
}

#[derive(Deserialize)]
pub struct ClaimRequest {
    pub claimant_did: String,
    pub claimant_pubkey_multibase: String,
    /// `ed25519:<base64>` over `CLAIM_DOMAIN || token || claimant_did_bytes || pubkey_bytes`.
    pub signature: String,
}

#[derive(Serialize)]
pub struct ClaimResponse {
    pub inviter_did: String,
    pub claimant_did: String,
    pub claimed_at: String,
    /// Whether the relay just registered the claimant as a tenant during
    /// claim (saves them a separate `sec daemon register` call).
    pub registered: bool,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRequest>,
) -> impl IntoResponse {
    let inviter = match Did::parse(&req.inviter_did) {
        Ok(d) => d,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid inviter_did: {e}")),
    };

    // Inviter must already be a registered tenant.
    let pubkey = match state.pubkey_for(&inviter) {
        Some(p) => p,
        None => {
            return error(
                StatusCode::FORBIDDEN,
                "inviter must register with this relay before creating an invite".into(),
            )
        }
    };

    let expires_at = match DateTime::parse_from_rfc3339(&req.expires_at) {
        Ok(t) => t.with_timezone(&Utc),
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("expires_at invalid: {e}")),
    };
    let now = Utc::now();
    let max_expiry = now + Duration::hours(MAX_TTL_HOURS);
    if expires_at <= now {
        return error(StatusCode::BAD_REQUEST, "expires_at is in the past".into());
    }
    let expires_at = expires_at.min(max_expiry);

    // Verify signature.
    let sig_bytes = match parse_ed25519_signature(&req.signature) {
        Ok(b) => b,
        Err(msg) => return error(StatusCode::BAD_REQUEST, msg),
    };
    let dalek_sig = DalekSig::from_bytes(&sig_bytes);

    let mut to_verify = CREATE_DOMAIN.to_vec();
    to_verify.extend_from_slice(inviter.as_str().as_bytes());
    to_verify.extend_from_slice(b":");
    to_verify.extend_from_slice(req.expires_at.as_bytes());
    to_verify.extend_from_slice(b":");
    if let Some(p) = req.purpose.as_deref() {
        to_verify.extend_from_slice(p.as_bytes());
    }

    if pubkey.verify(&to_verify, &dalek_sig).is_err() {
        return error(StatusCode::UNAUTHORIZED, "invite signature invalid".into());
    }

    let token = Uuid::new_v4().simple().to_string();
    let invite = Invite {
        token: token.clone(),
        inviter_did: inviter.clone(),
        created_at: now,
        expires_at,
        purpose: req.purpose,
        claimed_by: None,
        claimed_at: None,
    };
    state.create_invite(invite);
    info!(token = %token, inviter = %inviter, "invite created");

    (
        StatusCode::CREATED,
        Json(CreateResponse {
            token,
            expires_at: expires_at.to_rfc3339(),
        }),
    )
        .into_response()
}

pub async fn view(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    let invite = match state.get_invite(&token) {
        Some(i) => i,
        None => return error(StatusCode::NOT_FOUND, "invite not found".into()),
    };
    if invite.expires_at < Utc::now() && invite.claimed_by.is_none() {
        return error(StatusCode::GONE, "invite has expired".into());
    }
    Json(ViewResponse {
        inviter_did: invite.inviter_did.as_str().to_string(),
        expires_at: invite.expires_at.to_rfc3339(),
        purpose: invite.purpose,
        claimed_by: invite.claimed_by.map(|d| d.as_str().to_string()),
        install_url: "https://github.com/equanimitech/secretariat/releases/latest".into(),
    })
    .into_response()
}

pub async fn claim(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    Json(req): Json<ClaimRequest>,
) -> impl IntoResponse {
    let invite = match state.get_invite(&token) {
        Some(i) => i,
        None => return error(StatusCode::NOT_FOUND, "invite not found".into()),
    };
    if invite.claimed_by.is_some() {
        return error(StatusCode::CONFLICT, "invite has already been claimed".into());
    }
    if invite.expires_at < Utc::now() {
        return error(StatusCode::GONE, "invite has expired".into());
    }

    let claimant = match Did::parse(&req.claimant_did) {
        Ok(d) => d,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid claimant_did: {e}")),
    };

    let pubkey_bytes = match decode_ed25519_multibase(&req.claimant_pubkey_multibase) {
        Ok(b) => b,
        Err(e) => {
            return error(
                StatusCode::BAD_REQUEST,
                format!("invalid claimant_pubkey_multibase: {e}"),
            )
        }
    };
    let pubkey = match VerifyingKey::from_bytes(&pubkey_bytes) {
        Ok(p) => p,
        Err(e) => {
            return error(
                StatusCode::BAD_REQUEST,
                format!("invalid ed25519 pubkey: {e}"),
            )
        }
    };

    // Sanity for did:key — embedded key must match supplied pubkey.
    if claimant.method() == secretariat_core::domain::DidMethod::Key {
        match claimant.embedded_ed25519_key() {
            Some(embedded) if embedded == pubkey_bytes => {}
            _ => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "did:key embedded pubkey must match supplied pubkey".into(),
                )
            }
        }
    }

    let sig_bytes = match parse_ed25519_signature(&req.signature) {
        Ok(b) => b,
        Err(msg) => return error(StatusCode::BAD_REQUEST, msg),
    };
    let dalek_sig = DalekSig::from_bytes(&sig_bytes);

    let mut to_verify = CLAIM_DOMAIN.to_vec();
    to_verify.extend_from_slice(token.as_bytes());
    to_verify.extend_from_slice(b":");
    to_verify.extend_from_slice(claimant.as_str().as_bytes());
    to_verify.extend_from_slice(b":");
    to_verify.extend_from_slice(&pubkey_bytes);

    if pubkey.verify(&to_verify, &dalek_sig).is_err() {
        return error(StatusCode::UNAUTHORIZED, "claim signature invalid".into());
    }

    // Auto-register claimant if not yet known to the relay.
    let now = Utc::now();
    let registered = if !state.is_registered(&claimant) {
        state.register(claimant.clone(), pubkey, now);
        true
    } else {
        false
    };

    let claimed = match state.claim_invite(&token, claimant.clone(), now) {
        Some(i) => i,
        None => {
            return error(
                StatusCode::CONFLICT,
                "invite was just claimed by another peer".into(),
            )
        }
    };

    info!(token = %token, claimant = %claimant, "invite claimed");

    Json(ClaimResponse {
        inviter_did: claimed.inviter_did.as_str().to_string(),
        claimant_did: claimant.as_str().to_string(),
        claimed_at: now.to_rfc3339(),
        registered,
    })
    .into_response()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Bytes a client signs to create an invite. Public so client + tests
/// produce identical input.
pub fn create_input(inviter_did: &Did, expires_at: &str, purpose: Option<&str>) -> Vec<u8> {
    let mut v = CREATE_DOMAIN.to_vec();
    v.extend_from_slice(inviter_did.as_str().as_bytes());
    v.extend_from_slice(b":");
    v.extend_from_slice(expires_at.as_bytes());
    v.extend_from_slice(b":");
    if let Some(p) = purpose {
        v.extend_from_slice(p.as_bytes());
    }
    v
}

/// Bytes a client signs to claim an invite.
pub fn claim_input(token: &str, claimant_did: &Did, claimant_pubkey: &[u8; 32]) -> Vec<u8> {
    let mut v = CLAIM_DOMAIN.to_vec();
    v.extend_from_slice(token.as_bytes());
    v.extend_from_slice(b":");
    v.extend_from_slice(claimant_did.as_str().as_bytes());
    v.extend_from_slice(b":");
    v.extend_from_slice(claimant_pubkey);
    v
}

/// Default TTL when caller doesn't specify.
pub fn default_ttl_hours() -> i64 {
    DEFAULT_TTL_HOURS
}

fn parse_ed25519_signature(s: &str) -> Result<[u8; 64], String> {
    let body = s
        .strip_prefix("ed25519:")
        .ok_or_else(|| "signature must start with `ed25519:`".to_string())?;
    let bytes = B64
        .decode(body)
        .map_err(|e| format!("signature base64 invalid: {e}"))?;
    bytes
        .try_into()
        .map_err(|v: Vec<u8>| format!("signature must decode to 64 bytes, got {}", v.len()))
}

fn error(status: StatusCode, msg: String) -> axum::response::Response {
    (status, Json(ErrorBody { error: msg })).into_response()
}
