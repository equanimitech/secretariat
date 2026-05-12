//! Client side of the IPC protocol.
//!
//! Two flavours of caller in v0.3:
//!
//! - `sec daemon tick` (and the Tauri "Sync now" command, once migrated)
//!   want to *prefer* the running daemon but degrade to in-proc when no
//!   daemon is listening. Use [`tick_via_ipc_or_inproc`].
//! - Anything else that *requires* the daemon (future status fan-out)
//!   calls [`call`] directly and surfaces the connect failure.
//!
//! Connection model: one request, one response, close. Same shape as
//! the server.

use anyhow::{anyhow, Context, Result};
use ed25519_dalek::SigningKey;
use secretariat_core::application::SyncOutcome;
use secretariat_core::infrastructure::keys::KeyPaths;
use secretariat_core::Did;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use super::protocol::{Request, Response, JSONRPC_VERSION};
use super::socket_path;

/// Send a single JSON-RPC call and return the parsed result value.
/// Errors are surfaced as `anyhow::Error`.
pub async fn call(
    paths: &KeyPaths,
    method: &str,
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    let path = socket_path(paths);
    let stream = UnixStream::connect(&path)
        .await
        .with_context(|| format!("connecting to {}", path.display()))?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    let req = Request {
        jsonrpc: JSONRPC_VERSION.into(),
        id: 1,
        method: method.into(),
        params,
    };
    let mut line = serde_json::to_string(&req).context("encoding request")?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await?;
    writer.flush().await?;
    // Half-close so the server knows the request is complete even if
    // it's reading more than one line in a future protocol revision.
    drop(writer);

    let mut response_line = String::new();
    reader.read_line(&mut response_line).await?;
    if response_line.is_empty() {
        return Err(anyhow!("daemon closed connection without response"));
    }

    let resp: Response = serde_json::from_str(response_line.trim())
        .with_context(|| format!("decoding response: {response_line}"))?;
    if let Some(e) = resp.error {
        return Err(anyhow!("rpc error {}: {}", e.code, e.message));
    }
    Ok(resp.result.unwrap_or(serde_json::Value::Null))
}

/// Cheap probe: does a daemon appear to be listening on the IPC socket?
/// Doesn't send any payload; just opens the stream. False negatives
/// (daemon is up but slow to accept) are acceptable — callers fall back
/// to in-proc, which is correct behavior regardless.
pub async fn is_running(paths: &KeyPaths) -> bool {
    let path = socket_path(paths);
    UnixStream::connect(&path).await.is_ok()
}

/// Run a sync cycle. Tries the IPC socket first; falls back to running
/// the cycle in-process when no daemon is reachable. The fallback is
/// what preserves v0.2.16 behavior for users who haven't installed the
/// LaunchAgent or whose daemon is stopped.
pub async fn tick_via_ipc_or_inproc(
    paths: &KeyPaths,
    did: &Did,
    key: &SigningKey,
) -> Result<()> {
    if is_running(paths).await {
        let result = call(paths, "tick", None).await?;
        // The daemon already logged via tracing on its side; the
        // one-shot CLI invocation still needs visible feedback on
        // stderr. Try to deserialize the SyncOutcome and print the
        // shared summary; fall back to a generic line if the protocol
        // drifts so the user sees *something*.
        let summary = serde_json::from_value::<SyncOutcome>(result)
            .map(|o| crate::serve::summary_line(&o))
            .unwrap_or_else(|_| "[sec] tick complete".to_string());
        eprintln!("{summary}");
        Ok(())
    } else {
        // In-proc fallback: run the cycle here and surface the same
        // one-line summary. `tick_once`'s tracing logs stay structured
        // for daemon-loop callers but go silent without RUST_LOG, so
        // we reach into `run_tick` directly and handle both surfaces.
        let outcome = crate::serve::run_tick(paths, did, key).await?;
        crate::serve::log_outcome(&outcome);
        eprintln!("{}", crate::serve::summary_line(&outcome));
        Ok(())
    }
}
