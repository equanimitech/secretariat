//! `POST /v0/invite`                  — create a one-shot invite token (inviter signs)
//! `GET  /v0/invite/:token`           — public preview (no auth)
//! `POST /v0/invite/:token/claim`     — claimant signs + (optionally) self-registers
//! `GET  /v0/invites/claimed`         — inviter pulls list of claim events for
//!                                       bidirectional contact-add (bearer auth)
//!
//! Invites establish bilateral *correspondence* between two principals — see
//! `docs/milestones/2026-05-04-tauri-front-door.md` slice 2. Bidirectional
//! contact-add (the inviter learns the claimant) is the defining behavior,
//! not a side feature. The platform-install side-effect for not-yet-installed
//! claimers is incidental, handled by the relay-served HTML landing page on
//! `GET /v0/invite/:token` with `Accept: text/html`.

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
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let invite = match state.get_invite(&token) {
        Some(i) => i,
        None => return error(StatusCode::NOT_FOUND, "invite not found".into()),
    };
    if invite.expires_at < Utc::now() && invite.claimed_by.is_none() {
        return error(StatusCode::GONE, "invite has expired".into());
    }

    // Content negotiation: HTML for browsers, JSON for clients (default).
    // The HTML view lets a not-yet-installed claimer see the invite, learn
    // who's reaching out, click "Open in Secretariat" (deep link) or
    // "Install Secretariat" (GitHub release). The minimal landing is the
    // platform-install side-effect of the correspondence-invite primitive.
    let wants_html = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("text/html"))
        .unwrap_or(false);

    if wants_html {
        let host = headers
            .get(axum::http::header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let html = render_invite_html(&token, &invite, host);
        return ([(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")], html)
            .into_response();
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

/// Minimal HTML landing page for an invite. Single static page, no JS
/// framework, system fonts. Two affordances: "Open in Secretariat"
/// (deep link `secretariat://invite/<token>`) and "Install Secretariat"
/// (GitHub release). Lead with the relationship, not the platform —
/// invites are correspondence relationships, not platform onboarding.
fn render_invite_html(token: &str, invite: &crate::state::Invite, host: &str) -> String {
    let inviter_did = html_escape(invite.inviter_did.as_str());
    let purpose_block = match invite.purpose.as_deref() {
        Some(p) => format!(
            "<p class=\"purpose\">Purpose: <em>{}</em></p>",
            html_escape(p)
        ),
        None => String::new(),
    };
    let already_claimed_block = match invite.claimed_by.as_ref() {
        Some(claimer) => format!(
            "<p class=\"claimed\">This invite was already claimed by <code>{}</code>.</p>",
            html_escape(claimer.as_str())
        ),
        None => String::new(),
    };
    // Deep link mirrors the HTTPS claim URL with `secretariat://` in place
    // of `https://`. This way the deep link carries the relay endpoint,
    // so the app can claim against the right relay even when the user
    // has no relay registered yet.
    let deep_link = format!(
        "secretariat://{}/v0/invite/{}",
        html_escape(host),
        html_escape(token)
    );
    let install_url = "https://github.com/equanimitech/secretariat/releases/latest";

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Correspondence invite — Secretariat</title>
  <style>
    :root {{ color-scheme: light dark; }}
    html, body {{ height: 100%; }}
    body {{
      margin: 0;
      font: 16px/1.5 -apple-system, BlinkMacSystemFont, 'SF Pro Text', system-ui, sans-serif;
      display: grid; place-items: center;
      padding: 2rem;
      background: Canvas; color: CanvasText;
    }}
    main {{
      max-width: 32rem;
      width: 100%;
    }}
    h1 {{ font-weight: 600; font-size: 1.5rem; margin: 0 0 0.5rem; }}
    .lede {{ color: color-mix(in srgb, CanvasText 70%, transparent); margin: 0 0 1.5rem; }}
    code {{
      font: 0.85em ui-monospace, 'SF Mono', monospace;
      background: color-mix(in srgb, CanvasText 6%, transparent);
      padding: 0.1em 0.35em; border-radius: 4px;
    }}
    .purpose {{ font-size: 0.95rem; }}
    .claimed {{ color: color-mix(in srgb, CanvasText 60%, transparent); font-size: 0.9rem; }}
    .actions {{ display: flex; gap: 0.75rem; margin: 1.5rem 0 1rem; flex-wrap: wrap; }}
    .btn {{
      display: inline-block;
      padding: 0.6rem 1.1rem;
      border-radius: 8px;
      text-decoration: none;
      font-weight: 500;
      border: 1px solid color-mix(in srgb, CanvasText 20%, transparent);
    }}
    .btn-primary {{ background: CanvasText; color: Canvas; border-color: CanvasText; }}
    .footer {{
      margin-top: 2rem;
      font-size: 0.85rem;
      color: color-mix(in srgb, CanvasText 55%, transparent);
    }}
    .footer a {{ color: inherit; }}
  </style>
</head>
<body>
  <main>
    <h1>Someone wants to start a stamped correspondence with you</h1>
    <p class="lede">Inviter: <code>{inviter}</code></p>
    {purpose}
    {claimed}
    <div class="actions">
      <a class="btn btn-primary" href="{deep}">Open in Secretariat</a>
      <a class="btn" href="{install}">Install Secretariat</a>
    </div>
    <p class="footer">
      Secretariat is a cryptographically attested correspondence channel —
      AI drafts, humans stamp. Every envelope you exchange carries a
      biometric-attested signature. <a href="https://github.com/equanimitech/secretariat#readme">Learn more.</a>
    </p>
  </main>
</body>
</html>
"#,
        inviter = inviter_did,
        purpose = purpose_block,
        claimed = already_claimed_block,
        deep = html_escape(&deep_link),
        install = install_url,
    )
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
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

/// `GET /v0/invites/claimed` — inviter pulls list of claim events.
///
/// Returns every claimed invite where the authenticated principal is the
/// inviter, regardless of whether the daemon has seen it before. The
/// daemon dedupes against its local contact book (idempotent). No
/// relay-side ack state needed — keeps the relay stateless about the
/// inviter's processing progress.
#[derive(Serialize)]
pub struct ClaimedInvite {
    pub token: String,
    pub claimant_did: String,
    pub claimed_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
}

#[derive(Serialize)]
pub struct ClaimedListResponse {
    pub invites: Vec<ClaimedInvite>,
}

pub async fn list_claimed(
    State(state): State<std::sync::Arc<crate::state::AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let token = match headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
    {
        Some(t) => t.to_string(),
        None => return error(StatusCode::UNAUTHORIZED, "missing bearer token".into()),
    };

    let now = Utc::now();
    let inviter_did = match state.auth.validate_token(&token, now) {
        Ok(d) => d,
        Err(e) => return error(StatusCode::UNAUTHORIZED, e.to_string()),
    };

    let invites: Vec<ClaimedInvite> = state
        .invites_claimed_for_inviter(&inviter_did)
        .into_iter()
        .filter_map(|i| {
            let claimant = i.claimed_by?;
            let claimed_at = i.claimed_at?;
            Some(ClaimedInvite {
                token: i.token,
                claimant_did: claimant.as_str().to_string(),
                claimed_at: claimed_at.to_rfc3339(),
                purpose: i.purpose,
            })
        })
        .collect();

    Json(ClaimedListResponse { invites }).into_response()
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
