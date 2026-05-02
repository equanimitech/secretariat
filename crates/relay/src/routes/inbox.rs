//! `POST /v0/inbox/{did}` — anyone may queue an envelope for a registered DID.
//! `GET  /v0/inbox/{did}?after=<cursor>` — recipient pulls (bearer-auth).
//!
//! POST is intentionally open: senders' signatures are inside the envelope
//! (the recipient validates on receive); the relay's job is to queue bytes
//! for an addressed recipient. Spam protection at the relay is a v0.x concern
//! (rate limit by source IP, per-tenant accept-list, etc.).

use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use secretariat_core::Did;
use serde::{Deserialize, Serialize};

use crate::queue::QueuedEnvelope;
use crate::state::AppState;

const SENDER_HEADER: &str = "X-Sender-Did";

#[derive(Serialize)]
pub struct PostResponse {
    pub id: u64,
    pub queued_at: String,
}

#[derive(Deserialize, Default)]
pub struct PollQuery {
    #[serde(default)]
    pub after: u64,
}

#[derive(Serialize)]
pub struct PollResponse {
    pub envelopes: Vec<QueuedEnvelope>,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

pub async fn post(
    State(state): State<Arc<AppState>>,
    Path(recipient): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let recipient_did = match Did::parse(&recipient) {
        Ok(d) => d,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid recipient did: {e}")),
    };

    if !state.is_registered(&recipient_did) {
        return error(
            StatusCode::NOT_FOUND,
            "recipient is not registered with this relay".into(),
        );
    }

    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let sender_did = headers
        .get(SENDER_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Did::parse(s).ok());

    let now = Utc::now();
    let id = state.enqueue(recipient_did, body.to_vec(), content_type, sender_did, now);

    (
        StatusCode::ACCEPTED,
        Json(PostResponse {
            id,
            queued_at: now.to_rfc3339(),
        }),
    )
        .into_response()
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(recipient): Path<String>,
    Query(q): Query<PollQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let recipient_did = match Did::parse(&recipient) {
        Ok(d) => d,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid recipient did: {e}")),
    };

    let token = match bearer_token(&headers) {
        Some(t) => t,
        None => return error(StatusCode::UNAUTHORIZED, "missing bearer token".into()),
    };

    let now = Utc::now();
    let resolved_did = match state.auth.validate_token(&token, now) {
        Ok(d) => d,
        Err(e) => return error(StatusCode::UNAUTHORIZED, e.to_string()),
    };
    if resolved_did != recipient_did {
        return error(
            StatusCode::FORBIDDEN,
            "bearer token does not match recipient did".into(),
        );
    }

    let envelopes = state.since(&recipient_did, q.after);
    Json(PollResponse { envelopes }).into_response()
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

fn error(status: StatusCode, msg: String) -> axum::response::Response {
    (status, Json(ErrorBody { error: msg })).into_response()
}
