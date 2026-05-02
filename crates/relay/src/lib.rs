//! Secretariat relay — federation node for routing encrypted envelopes.
//!
//! See `docs/milestones/2026-05-02-v0-correspondence.md` for the v0 design
//! and `AGENTS.md` invariant #4 for the constraint that transports (this
//! relay included) see signed-and-encrypted bytes only — never plaintext,
//! never envelope structure beyond outermost addressing.
//!
//! The relay is *not* a central server. It is a federation node: each
//! principal hosts their own (or uses one a peer hosts for them). The
//! authority for verification stays with the recipient via DID resolution.

pub mod auth;
pub mod config;
pub mod queue;
pub mod routes;
pub mod state;

use std::sync::Arc;

use axum::{routing::get, routing::post, Router};
use tower_http::trace::TraceLayer;

pub use config::{Config, QueueTtlDays, RegistrationPolicy};
pub use state::AppState;

/// Build the axum router with all v0 routes wired in.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(routes::health::handler))
        .route("/v0/register", post(routes::register::handler))
        .route("/v0/auth/challenge", post(routes::auth::challenge))
        .route("/v0/auth/answer", post(routes::auth::answer))
        .route(
            "/v0/inbox/:did",
            post(routes::inbox::post).get(routes::inbox::get),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
