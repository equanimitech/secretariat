//! `POST /v0/auth/challenge` and `POST /v0/auth/answer` — bearer token issue.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chrono::{Duration, Utc};
use secretariat_core::Did;
use serde::{Deserialize, Serialize};

use crate::auth::{AuthState, SESSION_TTL_SECS};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ChallengeRequest {
    pub did: String,
}

#[derive(Serialize)]
pub struct ChallengeResponse {
    pub nonce: String,
    pub expires_at: String,
}

#[derive(Deserialize)]
pub struct AnswerRequest {
    pub did: String,
    pub nonce: String,
    pub signature: String, // ed25519:<base64>
}

#[derive(Serialize)]
pub struct AnswerResponse {
    pub token: String,
    pub expires_at: String,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

pub async fn challenge(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChallengeRequest>,
) -> impl IntoResponse {
    let did = match Did::parse(&req.did) {
        Ok(d) => d,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid did: {e}")),
    };
    if !state.is_registered(&did) {
        return error(StatusCode::NOT_FOUND, "did is not registered".into());
    }
    let now = Utc::now();
    let nonce = state.auth.issue_challenge(did, now);
    let expires_at = (now + Duration::seconds(60)).to_rfc3339();
    Json(ChallengeResponse { nonce, expires_at }).into_response()
}

pub async fn answer(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AnswerRequest>,
) -> impl IntoResponse {
    let did = match Did::parse(&req.did) {
        Ok(d) => d,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid did: {e}")),
    };
    let pubkey = match state.pubkey_for(&did) {
        Some(p) => p,
        None => return error(StatusCode::NOT_FOUND, "did is not registered".into()),
    };

    let sig_bytes = match parse_ed25519_signature(&req.signature) {
        Ok(b) => b,
        Err(msg) => return error(StatusCode::BAD_REQUEST, msg),
    };

    let now = Utc::now();
    let token = match state
        .auth
        .verify_and_issue_token(&did, &req.nonce, &sig_bytes, &pubkey, now)
    {
        Ok(t) => t,
        Err(e) => return error(StatusCode::UNAUTHORIZED, e.to_string()),
    };
    let expires_at = (now + Duration::seconds(SESSION_TTL_SECS)).to_rfc3339();
    Json(AnswerResponse { token, expires_at }).into_response()
}

fn parse_ed25519_signature(s: &str) -> Result<Vec<u8>, String> {
    let body = s
        .strip_prefix("ed25519:")
        .ok_or_else(|| "signature must start with `ed25519:`".to_string())?;
    B64.decode(body)
        .map_err(|e| format!("signature base64 invalid: {e}"))
}

/// Re-export the canonical input so client + tests can reproduce it.
pub fn auth_input(nonce: &str) -> Vec<u8> {
    AuthState::auth_input(nonce)
}

fn error(status: StatusCode, msg: String) -> axum::response::Response {
    (status, Json(ErrorBody { error: msg })).into_response()
}
