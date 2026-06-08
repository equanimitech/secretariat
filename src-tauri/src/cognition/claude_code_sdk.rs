//! Adapter implementing `CognitionSession` over the `cognition-claude-sdk`
//! Bun-compiled sidecar that wraps `@anthropic-ai/claude-agent-sdk`.
//!
//! Architecture: one sidecar process per app lifetime; commands and
//! events multiplexed by `session_id`. The adapter holds the stdin
//! writer, a stdout reader task that fans events out to per-session
//! channels, and a sinks map updated on each turn.

use std::collections::HashMap;
use std::sync::Arc;

use secretariat_core::ports::{CognitionSession, SessionError, SessionEvent, SessionRef};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::sync::{oneshot, RwLock};

pub struct ClaudeCodeSdkAdapter {
    cmd_tx: UnboundedSender<String>,
    sinks: Arc<RwLock<HashMap<String, SessionSink>>>,
}

struct SessionSink {
    events: UnboundedSender<SessionEvent>,
    completion: Option<oneshot::Sender<Result<(), SessionError>>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Outbound<'a> {
    Send {
        session_id: &'a str,
        channel_dir: &'a str,
        message: &'a str,
        is_first_turn: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<&'a str>,
    },
    Cancel {
        session_id: &'a str,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Inbound {
    Ready,
    TextDelta {
        session_id: String,
        text: String,
    },
    ToolCallStart {
        session_id: String,
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolCallResult {
        session_id: String,
        id: String,
        output: serde_json::Value,
    },
    Thinking {
        session_id: String,
        text: String,
    },
    Warning {
        session_id: String,
        message: String,
    },
    Done {
        session_id: String,
        stop_reason: String,
    },
    Error {
        session_id: String,
        message: String,
    },
}

impl ClaudeCodeSdkAdapter {
    /// Spawn the sidecar and wire up the I/O loops.
    pub fn spawn(app: &AppHandle) -> Result<Self, SessionError> {
        let mut sidecar = app
            .shell()
            .sidecar("cognition-claude-sdk")
            .map_err(|e| SessionError::Unavailable(format!("sidecar lookup: {e}")))?;

        // Pin sec-mcp + claude binary paths so the SDK has Secretariat
        // tools available in every session (per [[project_mcp_is_primary_interface]]).
        if let Some(sec_mcp) = resolve_sec_mcp_path() {
            sidecar = sidecar.env(
                "SECRETARIAT_SEC_MCP_PATH",
                sec_mcp.to_string_lossy().as_ref(),
            );
        }
        if let Some(claude) = resolve_claude_path() {
            sidecar = sidecar.env("SECRETARIAT_CLAUDE_PATH", claude.to_string_lossy().as_ref());
        }

        let (mut rx, mut child) = sidecar
            .spawn()
            .map_err(|e| SessionError::Unavailable(format!("spawn sidecar: {e}")))?;

        let (cmd_tx, mut cmd_rx) = unbounded_channel::<String>();
        let sinks: Arc<RwLock<HashMap<String, SessionSink>>> =
            Arc::new(RwLock::new(HashMap::new()));

        // Writer task — drains command channel, writes JSON lines to sidecar stdin.
        tauri::async_runtime::spawn(async move {
            while let Some(line) = cmd_rx.recv().await {
                let mut payload = line.into_bytes();
                payload.push(b'\n');
                if let Err(e) = child.write(&payload) {
                    log::warn!("cognition sidecar stdin write failed: {e}");
                    break;
                }
            }
        });

        // Reader task — drains sidecar stdout, parses JSON, dispatches to sinks.
        let sinks_for_reader = Arc::clone(&sinks);
        tauri::async_runtime::spawn(async move {
            let mut buf = String::new();
            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(bytes) => {
                        buf.push_str(&String::from_utf8_lossy(&bytes));
                        while let Some(idx) = buf.find('\n') {
                            let line = buf[..idx].trim_end_matches('\r').to_string();
                            buf.drain(..=idx);
                            if line.is_empty() {
                                continue;
                            }
                            match serde_json::from_str::<Inbound>(&line) {
                                Ok(inbound) => {
                                    dispatch(&sinks_for_reader, inbound).await;
                                }
                                Err(e) => {
                                    log::warn!("cognition sidecar: unparseable line {line:?}: {e}");
                                }
                            }
                        }
                    }
                    CommandEvent::Stderr(bytes) => {
                        log::warn!(
                            "cognition sidecar stderr: {}",
                            String::from_utf8_lossy(&bytes).trim_end()
                        );
                    }
                    CommandEvent::Terminated(payload) => {
                        log::error!("cognition sidecar exited code={:?}", payload.code);
                        // Fail-fast every outstanding sink.
                        let mut guard = sinks_for_reader.write().await;
                        for (_, mut sink) in guard.drain() {
                            if let Some(c) = sink.completion.take() {
                                let _ = c.send(Err(SessionError::Unavailable(
                                    "sidecar terminated".into(),
                                )));
                            }
                        }
                        break;
                    }
                    CommandEvent::Error(err) => {
                        log::error!("cognition sidecar error: {err}");
                    }
                    _ => {}
                }
            }
        });

        Ok(Self { cmd_tx, sinks })
    }
}

async fn dispatch(sinks: &Arc<RwLock<HashMap<String, SessionSink>>>, msg: Inbound) {
    let session_id_opt: Option<&str> = match &msg {
        Inbound::Ready => None,
        Inbound::TextDelta { session_id, .. }
        | Inbound::ToolCallStart { session_id, .. }
        | Inbound::ToolCallResult { session_id, .. }
        | Inbound::Thinking { session_id, .. }
        | Inbound::Warning { session_id, .. }
        | Inbound::Done { session_id, .. }
        | Inbound::Error { session_id, .. } => Some(session_id.as_str()),
    };

    let Some(session_id) = session_id_opt else {
        return;
    };
    if session_id.is_empty() {
        log::warn!("cognition sidecar: event with empty session_id: {msg:?}");
        return;
    }

    match msg {
        Inbound::Ready => {}
        Inbound::TextDelta { session_id, text } => {
            forward_event(sinks, &session_id, SessionEvent::TextDelta(text)).await;
        }
        Inbound::ToolCallStart {
            session_id,
            id,
            name,
            input,
        } => {
            forward_event(
                sinks,
                &session_id,
                SessionEvent::ToolCallStart { id, name, input },
            )
            .await;
        }
        Inbound::ToolCallResult {
            session_id,
            id,
            output,
        } => {
            forward_event(
                sinks,
                &session_id,
                SessionEvent::ToolCallResult { id, output },
            )
            .await;
        }
        Inbound::Thinking { session_id, text } => {
            forward_event(sinks, &session_id, SessionEvent::Thinking(text)).await;
        }
        Inbound::Warning {
            session_id,
            message,
        } => {
            forward_event(sinks, &session_id, SessionEvent::Warning(message)).await;
        }
        Inbound::Done {
            session_id,
            stop_reason,
        } => {
            let sink = sinks.write().await.remove(&session_id);
            if let Some(mut sink) = sink {
                let _ = sink.events.send(SessionEvent::Done {
                    stop_reason: stop_reason.clone(),
                });
                if let Some(c) = sink.completion.take() {
                    let _ = c.send(Ok(()));
                }
            }
        }
        Inbound::Error {
            session_id,
            message,
        } => {
            let sink = sinks.write().await.remove(&session_id);
            if let Some(mut sink) = sink {
                let _ = sink.events.send(SessionEvent::Warning(message.clone()));
                if let Some(c) = sink.completion.take() {
                    let _ = c.send(Err(SessionError::Refused(message)));
                }
            }
        }
    }
}

async fn forward_event(
    sinks: &Arc<RwLock<HashMap<String, SessionSink>>>,
    session_id: &str,
    event: SessionEvent,
) {
    let guard = sinks.read().await;
    if let Some(sink) = guard.get(session_id) {
        let _ = sink.events.send(event);
    }
}

impl CognitionSession for ClaudeCodeSdkAdapter {
    fn send_turn(
        &self,
        session: &SessionRef,
        message: String,
        events: UnboundedSender<SessionEvent>,
    ) -> impl std::future::Future<Output = Result<(), SessionError>> + Send {
        let session_id = session.session_id.clone();
        let channel_dir = session.channel_dir.to_string_lossy().into_owned();
        let is_first_turn = session.is_first_turn;
        let model = session.model.clone();
        let cmd_tx = self.cmd_tx.clone();
        let sinks = Arc::clone(&self.sinks);

        async move {
            let (completion_tx, completion_rx) = oneshot::channel();
            {
                let mut guard = sinks.write().await;
                guard.insert(
                    session_id.clone(),
                    SessionSink {
                        events,
                        completion: Some(completion_tx),
                    },
                );
            }

            let line = serde_json::to_string(&Outbound::Send {
                session_id: &session_id,
                channel_dir: &channel_dir,
                message: &message,
                is_first_turn,
                model: model.as_deref(),
            })
            .map_err(|e| SessionError::Internal(format!("encode send: {e}")))?;
            cmd_tx
                .send(line)
                .map_err(|_| SessionError::Unavailable("sidecar channel closed".into()))?;

            completion_rx
                .await
                .map_err(|_| SessionError::Unavailable("completion dropped".into()))?
        }
    }

    fn cancel(
        &self,
        session_id: &str,
    ) -> impl std::future::Future<Output = Result<(), SessionError>> + Send {
        let session_id = session_id.to_string();
        let cmd_tx = self.cmd_tx.clone();
        async move {
            let line = serde_json::to_string(&Outbound::Cancel {
                session_id: &session_id,
            })
            .map_err(|e| SessionError::Internal(format!("encode cancel: {e}")))?;
            cmd_tx
                .send(line)
                .map_err(|_| SessionError::Unavailable("sidecar channel closed".into()))?;
            Ok(())
        }
    }
}

/// Manage the singleton adapter as Tauri state. Called from `lib.rs` setup
/// after the AppHandle is available.
pub fn install_into(app: &AppHandle) -> Result<(), String> {
    let adapter =
        ClaudeCodeSdkAdapter::spawn(app).map_err(|e| format!("spawn cognition sidecar: {e}"))?;
    app.manage(Arc::new(adapter));
    Ok(())
}

/// Locate the `sec-mcp` binary the SDK should spawn for Secretariat tools.
/// Prefers the bundled sidecar next to the running app exe (release builds);
/// falls back to `sec-mcp` on PATH for dev runs.
fn resolve_sec_mcp_path() -> Option<std::path::PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("sec-mcp");
            if bundled.exists() {
                return Some(bundled);
            }
        }
    }
    if let Ok(out) = std::process::Command::new("which").arg("sec-mcp").output() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(std::path::PathBuf::from(s));
        }
    }
    None
}

/// Locate the standalone `claude` executable to override the SDK's bundled
/// cli.js (the bundled path lives in Bun's virtual FS and isn't reachable
/// after `bun build --compile`).
pub(crate) fn resolve_claude_path() -> Option<std::path::PathBuf> {
    if let Ok(out) = std::process::Command::new("which").arg("claude").output() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(std::path::PathBuf::from(s));
        }
    }
    None
}
