//! Unix-socket listener task spawned by [`crate::serve::serve`].
//!
//! Lifecycle:
//! 1. Compute socket path under `paths.root`. Delete any stale file at
//!    that path (a previous daemon that didn't clean up on exit).
//! 2. `bind()` a [`UnixListener`]. On failure (another daemon already
//!    listening, filesystem refuses sockets), log a warning and return
//!    — the poll loop keeps serving its primary duty.
//! 3. `chmod 0600` the socket so only the principal's UID can connect.
//! 4. Accept loop: each connection spawned onto its own task. Per-
//!    connection failures are logged; the listener stays up.
//!
//! Each connection: read one JSON line, parse, dispatch on `method`,
//! write one JSON line back, close. Multiplexed clients open new
//! connections per call — keeps the protocol stateless. Push
//! subscriptions (Slice 5) will use a different framing on a different
//! method.
//!
//! [`UnixListener`]: tokio::net::UnixListener

use std::path::PathBuf;

use ed25519_dalek::SigningKey;
use secretariat_core::infrastructure::keys::KeyPaths;
use secretariat_core::Did;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use super::protocol::{codes, Request, Response};
use super::socket_path;

pub fn spawn_server(paths: KeyPaths, did: Did, key: SigningKey) -> JoinHandle<()> {
    tokio::spawn(async move {
        let path = socket_path(&paths);
        if let Err(e) = run(path.clone(), paths, did, key).await {
            warn!(socket = %path.display(), error = %e, "ipc server stopped");
        }
    })
}

async fn run(path: PathBuf, paths: KeyPaths, did: Did, key: SigningKey) -> anyhow::Result<()> {
    // Stale-socket cleanup. If a prior daemon crashed without removing
    // the file, bind will fail with EADDRINUSE — even though nothing is
    // listening. Try connecting first: if a connection succeeds, another
    // daemon owns it and we step aside. If it doesn't, the file is dead.
    //
    // TOCTOU note: between the `remove_file` and the `bind` below, a
    // second daemon racing to start could theoretically bind the same
    // path first. Acceptable for v0.3's single-user / single-machine
    // deployment — `KeepAlive` doesn't race against manual starts under
    // normal use. Revisit if multi-instance ever becomes a real config.
    if path.exists() {
        match UnixStream::connect(&path).await {
            Ok(_) => {
                warn!(
                    socket = %path.display(),
                    "another daemon appears to own the IPC socket; not starting a second listener"
                );
                return Ok(());
            }
            Err(_) => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    let listener = UnixListener::bind(&path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }

    info!(socket = %path.display(), "ipc socket listening");

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let paths = paths.clone();
                let did = did.clone();
                let key = key.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, &paths, &did, &key).await {
                        warn!(error = %e, "ipc connection error");
                    }
                });
            }
            Err(e) => {
                warn!(error = %e, "ipc accept error");
                // Brief backoff so a wedged accept loop doesn't spin.
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

async fn handle_connection(
    stream: UnixStream,
    paths: &KeyPaths,
    did: &Did,
    key: &SigningKey,
) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    let mut buf = String::new();
    let n = reader.read_line(&mut buf).await?;
    if n == 0 {
        return Ok(()); // client closed without sending
    }

    let response = match serde_json::from_str::<Request>(buf.trim()) {
        Ok(req) => dispatch(req, paths, did, key).await,
        Err(e) => Response::err(0, codes::PARSE_ERROR, format!("parse error: {e}")),
    };

    let mut line = serde_json::to_string(&response)?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn dispatch(req: Request, paths: &KeyPaths, did: &Did, key: &SigningKey) -> Response {
    match req.method.as_str() {
        "version" => Response::ok(
            req.id,
            serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }),
        ),
        "ping" => Response::ok(req.id, serde_json::json!({ "ok": true })),
        "tick" => match crate::serve::run_tick(paths, did, key).await {
            Ok(outcome) => match serde_json::to_value(&outcome) {
                Ok(v) => {
                    crate::serve::log_outcome(&outcome);
                    Response::ok(req.id, v)
                }
                Err(e) => Response::err(
                    req.id,
                    codes::INTERNAL_ERROR,
                    format!("serialize outcome: {e}"),
                ),
            },
            Err(e) => Response::err(req.id, codes::INTERNAL_ERROR, e.to_string()),
        },
        other => Response::err(
            req.id,
            codes::METHOD_NOT_FOUND,
            format!("unknown method: {other}"),
        ),
    }
}
