//! `POST /v0/register` — tenant registers their DID + ed25519 pubkey.
//!
//! Request body (JSON):
//!
//! ```json
//! {
//!   "did": "did:key:z..." | "did:web:...",
//!   "pubkey_multibase": "z...",
//!   "signature": "ed25519:<base64>"
//! }
//! ```
//!
//! The signature is over the byte concatenation
//! `b"secretariat-relay-register:v0:" || did_bytes || pubkey_bytes`.
//! For `did:key` the relay also asserts the embedded pubkey matches the
//! supplied one (defense in depth — the client could otherwise lie about
//! their pubkey at registration time, though they would then be unable to
//! satisfy the auth challenge later).
//!
//! Allowlist mode (configured by `--allowlist`) restricts registration to
//! the listed DIDs.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::Utc;
use ed25519_dalek::{Signature as DalekSig, Verifier, VerifyingKey};
use secretariat_core::codec;
use secretariat_core::domain::DidMethod;
use secretariat_core::Did;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::config::RegistrationPolicy;
use crate::state::AppState;

const REGISTER_DOMAIN: &[u8] = b"secretariat-relay-register:v0:";

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub did: String,
    pub pubkey_multibase: String,
    pub signature: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub did: String,
    pub registered_at: String,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    let did = match Did::parse(&req.did) {
        Ok(d) => d,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid did: {e}")),
    };

    // Allowlist gate.
    if let RegistrationPolicy::Allowlist(allow) = &state.config.registration {
        if !allow.contains(&did) {
            return error(
                StatusCode::FORBIDDEN,
                "this relay is in allowlist mode; the supplied DID is not permitted".into(),
            );
        }
    }

    // Decode the supplied pubkey.
    let pubkey_bytes = match codec::decode_ed25519_multibase(&req.pubkey_multibase) {
        Ok(b) => b,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid pubkey_multibase: {e}")),
    };
    let pubkey = match VerifyingKey::from_bytes(&pubkey_bytes) {
        Ok(p) => p,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid ed25519 pubkey: {e}")),
    };

    // For did:key, ensure the supplied pubkey actually matches what's embedded.
    if did.method() == DidMethod::Key {
        match did.embedded_ed25519_key() {
            Some(embedded) if embedded == pubkey_bytes => {}
            Some(_) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "did:key embedded pubkey does not match supplied pubkey".into(),
                )
            }
            None => return error(StatusCode::BAD_REQUEST, "could not extract did:key pubkey".into()),
        }
    }

    // Decode and verify signature.
    let sig_bytes = match parse_ed25519_signature(&req.signature) {
        Ok(b) => b,
        Err(msg) => return error(StatusCode::BAD_REQUEST, msg),
    };
    let dalek_sig = DalekSig::from_bytes(&sig_bytes);

    let mut to_verify = REGISTER_DOMAIN.to_vec();
    to_verify.extend_from_slice(did.as_str().as_bytes());
    to_verify.extend_from_slice(&pubkey_bytes);

    if pubkey.verify(&to_verify, &dalek_sig).is_err() {
        return error(StatusCode::UNAUTHORIZED, "registration signature invalid".into());
    }

    // Reject duplicate registration. v0 doesn't support key rotation here.
    if state.is_registered(&did) {
        return error(StatusCode::CONFLICT, "DID already registered".into());
    }

    let now = Utc::now();
    let tenant = state.register(did.clone(), pubkey, now);
    info!(did = %did, "registered tenant");

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "did": tenant.did.as_str(),
            "registered_at": tenant.registered_at.to_rfc3339(),
        })),
    )
        .into_response()
}

fn parse_ed25519_signature(s: &str) -> Result<[u8; 64], String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
    let body = s
        .strip_prefix("ed25519:")
        .ok_or_else(|| "signature must start with `ed25519:`".to_string())?;
    let bytes = B64
        .decode(body)
        .map_err(|e| format!("signature base64 invalid: {e}"))?;
    bytes.try_into().map_err(|v: Vec<u8>| {
        format!("signature must decode to 64 bytes, got {}", v.len())
    })
}

/// What a client must sign to register. Public so client adapters / tests can
/// produce the exact same bytes.
pub fn registration_input(did: &Did, pubkey_bytes: &[u8; 32]) -> Vec<u8> {
    let mut v = REGISTER_DOMAIN.to_vec();
    v.extend_from_slice(did.as_str().as_bytes());
    v.extend_from_slice(pubkey_bytes);
    v
}

fn error(status: StatusCode, msg: String) -> axum::response::Response {
    (status, Json(ErrorBody { error: msg })).into_response()
}
