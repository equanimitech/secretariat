//! `GET /healthz` — for Railway / load balancer healthchecks.

use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub version: &'static str,
    pub registered_count: usize,
    pub queue_count: usize,
}

pub async fn handler(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
        registered_count: state.registered_count(),
        queue_count: state.channel_queue_lengths().len(),
    })
}
