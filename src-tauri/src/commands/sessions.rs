//! Channel-scoped Claude sessions powering the tab strip primary UI.
//!
//! Each tab is one Claude conversation pinned to a channel-dir
//! (per [[project_channel_dir_is_activation_surface]]). Turns flow
//! through the wired `CognitionSession` adapter — today
//! `ClaudeCodeSdkAdapter` (Bun-compiled @anthropic-ai/claude-agent-sdk
//! sidecar). The adapter persists conversation state to its substrate's
//! native location; this module is transport-only.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use secretariat_core::ports::{CognitionSession, SessionEvent, SessionRef};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc::unbounded_channel;

use crate::cognition::claude_code_sdk::ClaudeCodeSdkAdapter;

/// Per-tab cancel handle — set when a turn starts, cleared on done.
#[derive(Default)]
pub struct SessionState {
    in_flight: Mutex<HashMap<String, String>>, // tab_id -> session_id
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SessionStreamDelta {
    pub tab_id: String,
    pub kind: String,
    pub payload: serde_json::Value,
}

/// Send one user message into a channel-scoped Claude session.
///
/// `session_id` is the caller-supplied stable handle. First turn for a
/// new conversation passes `is_first_turn=true`; the adapter generates
/// substrate state. Subsequent turns reuse the same `session_id` with
/// `is_first_turn=false` to resume.
#[tauri::command]
#[specta::specta]
pub async fn session_send(
    app: AppHandle,
    state: State<'_, SessionState>,
    tab_id: String,
    channel_path: String,
    session_id: String,
    message: String,
    is_first_turn: bool,
) -> Result<(), String> {
    let adapter: Arc<ClaudeCodeSdkAdapter> = app
        .try_state::<Arc<ClaudeCodeSdkAdapter>>()
        .map(|s| Arc::clone(&*s))
        .ok_or_else(|| "cognition adapter not installed".to_string())?;

    let session = SessionRef {
        session_id: session_id.clone(),
        channel_dir: PathBuf::from(&channel_path),
        is_first_turn,
        model: None,
    };

    let (tx, mut rx) = unbounded_channel::<SessionEvent>();
    {
        let mut map = state.in_flight.lock().unwrap();
        map.insert(tab_id.clone(), session_id.clone());
    }

    // Drain events → Tauri events on a separate task so the await below
    // resolves only when the adapter finishes the turn.
    let app_for_events = app.clone();
    let tab_for_events = tab_id.clone();
    let event_name = format!("session-stream:{tab_id}");
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            let (kind, payload) = serialize_event(event);
            let _ = app_for_events.emit(
                &event_name,
                SessionStreamDelta {
                    tab_id: tab_for_events.clone(),
                    kind,
                    payload,
                },
            );
        }
    });

    let result = adapter.send_turn(&session, message, tx).await;

    {
        let mut map = state.in_flight.lock().unwrap();
        map.remove(&tab_id);
    }

    result.map_err(|e| format!("send_turn: {e}"))
}

/// Cancel an in-flight turn. Idempotent.
#[tauri::command]
#[specta::specta]
pub async fn session_cancel(
    app: AppHandle,
    state: State<'_, SessionState>,
    tab_id: String,
) -> Result<(), String> {
    let session_id = {
        let mut map = state.in_flight.lock().unwrap();
        map.remove(&tab_id)
    };
    let Some(session_id) = session_id else {
        return Ok(());
    };
    let adapter: Arc<ClaudeCodeSdkAdapter> = app
        .try_state::<Arc<ClaudeCodeSdkAdapter>>()
        .map(|s| Arc::clone(&*s))
        .ok_or_else(|| "cognition adapter not installed".to_string())?;
    adapter
        .cancel(&session_id)
        .await
        .map_err(|e| format!("cancel: {e}"))
}

fn serialize_event(event: SessionEvent) -> (String, serde_json::Value) {
    match event {
        SessionEvent::TextDelta(text) => ("text_delta".into(), serde_json::json!({ "text": text })),
        SessionEvent::ToolCallStart { id, name, input } => (
            "tool_call_start".into(),
            serde_json::json!({ "id": id, "name": name, "input": input }),
        ),
        SessionEvent::ToolCallResult { id, output } => (
            "tool_call_result".into(),
            serde_json::json!({ "id": id, "output": output }),
        ),
        SessionEvent::Thinking(text) => ("thinking".into(), serde_json::json!({ "text": text })),
        SessionEvent::Warning(message) => {
            ("warning".into(), serde_json::json!({ "message": message }))
        }
        SessionEvent::Done { stop_reason } => (
            "done".into(),
            serde_json::json!({ "stop_reason": stop_reason }),
        ),
    }
}
