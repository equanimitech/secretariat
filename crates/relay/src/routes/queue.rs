//! `POST /v0/queue/{owner_did}/{handle}` — anyone may publish an envelope
//! to a queue owned by a registered DID.
//! `GET  /v0/queue/{owner_did}/{handle}?after=<cursor>` — a registered
//! caller pulls the channel's stream (bearer-auth on the caller).
//!
//! Generalizes `/v0/inbox/:did`, which is the two-party case (handle
//! `inbox:default`). The handle path-param is single-segment + URL-encoded
//! (axum decodes percent-encoding into the extracted `String`); colons in
//! handles travel as `%3A` on the wire.
//!
//! Auth shape (v0.8 dev — no roster gate yet):
//! - POST is open. Senders' signatures live inside the envelope body; the
//!   relay's job is to queue bytes for an `(owner, handle)`.
//! - GET requires a bearer token. The caller's DID (resolved from the
//!   token) must be registered with this relay. This is weaker than the
//!   final roster gate from the pitch, but matches the "ship minimum,
//!   layer security after" stance for first channel traffic.

use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use secretariat_core::domain::QueueHandle;
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
    Path((owner, handle)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let owner_did = match Did::parse(&owner) {
        Ok(d) => d,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid owner did: {e}")),
    };
    let handle = match QueueHandle::parse(&handle) {
        Ok(h) => h,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid handle: {e}")),
    };

    if !state.is_registered(&owner_did) {
        return error(
            StatusCode::NOT_FOUND,
            "channel owner is not registered with this relay".into(),
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
    let id = state.enqueue(owner_did, handle, body.to_vec(), content_type, sender_did, now);

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
    Path((owner, handle)): Path<(String, String)>,
    Query(q): Query<PollQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let owner_did = match Did::parse(&owner) {
        Ok(d) => d,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid owner did: {e}")),
    };
    let handle = match QueueHandle::parse(&handle) {
        Ok(h) => h,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid handle: {e}")),
    };

    let token = match bearer_token(&headers) {
        Some(t) => t,
        None => return error(StatusCode::UNAUTHORIZED, "missing bearer token".into()),
    };

    let now = Utc::now();
    let caller_did = match state.auth.validate_token(&token, now) {
        Ok(d) => d,
        Err(e) => return error(StatusCode::UNAUTHORIZED, e.to_string()),
    };
    if !state.is_registered(&caller_did) {
        return error(StatusCode::FORBIDDEN, "caller is not registered".into());
    }

    let envelopes = state.since(&owner_did, &handle, q.after);
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
