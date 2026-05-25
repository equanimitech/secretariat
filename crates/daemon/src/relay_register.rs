//! One-time relay registration. Called by `sec daemon register --endpoint
//! <url>` after the principal has decided which relay hosts their
//! correspondence inbox.
//!
//! Side effects: writes to `RelayState` at `paths.relay_state` and prints
//! to stderr. The signing key is borrowed only for the registration
//! request; nothing is retained inside the daemon crate.

use anyhow::{Context, Result};
use ed25519_dalek::SigningKey;
use secretariat_core::infrastructure::keys::KeyPaths;
use secretariat_core::infrastructure::transport::{RelayClient, RelayState};
use secretariat_core::Did;

pub async fn register(paths: &KeyPaths, did: &Did, key: &SigningKey, endpoint: &str) -> Result<()> {
    let client = RelayClient::new(endpoint, did.clone(), key);
    client.register().await.context("relay registration")?;

    let mut state = RelayState::load(&paths.relay_state).context("loading relay state")?;
    let entry = state.entry_mut(client.endpoint.as_str());
    entry.registered = true;
    state
        .save(&paths.relay_state)
        .context("saving relay state")?;

    eprintln!("[sec] registered with {}", client.endpoint);
    eprintln!("[sec]   did: {did}");
    Ok(())
}
