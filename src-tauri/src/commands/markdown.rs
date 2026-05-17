//! Tauri commands for the markdown reader/editor.
//!
//! Thin IPC wrapper over `crate::markdown::{file_io, pending}`.

use crate::markdown::{
    file_io::{self, WriteError},
    pending::PendingOpens,
};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use specta::Type;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

#[derive(Serialize, Type)]
pub struct ReadMarkdownResult {
    pub content: String,
    pub sha256: String,
}

#[derive(Deserialize, Type)]
pub struct WriteMarkdownArgs {
    pub path: String,
    pub content: String,
    pub expected_sha256: String,
}

#[derive(Serialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WriteMarkdownResult {
    Ok { sha256: String },
    Conflict { current_sha256: String },
}

#[tauri::command]
#[specta::specta]
pub fn read_markdown(path: String) -> Result<ReadMarkdownResult, String> {
    let r = file_io::read_file(&PathBuf::from(&path)).map_err(|e| e.to_string())?;
    Ok(ReadMarkdownResult {
        content: r.content,
        sha256: r.sha256,
    })
}

#[tauri::command]
#[specta::specta]
pub fn write_markdown(args: WriteMarkdownArgs) -> Result<WriteMarkdownResult, String> {
    match file_io::write_file(
        &PathBuf::from(&args.path),
        &args.content,
        &args.expected_sha256,
    ) {
        Ok(sha256) => Ok(WriteMarkdownResult::Ok { sha256 }),
        Err(WriteError::Conflict { current_sha256 }) => {
            Ok(WriteMarkdownResult::Conflict { current_sha256 })
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn open_markdown_window(app: AppHandle, path: String) -> Result<String, String> {
    let label = window_label(&path);
    if let Some(existing) = app.get_webview_window(&label) {
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(label);
    }
    let encoded = urlencoding::encode(&path);
    let url = format!("markdown-window.html?path={encoded}");
    WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title("Markdown")
        .inner_size(1100.0, 820.0)
        .min_inner_size(560.0, 420.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(label)
}

#[tauri::command]
#[specta::specta]
pub fn take_pending_opens(pending: State<'_, PendingOpens>) -> Vec<String> {
    pending
        .drain()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect()
}

fn window_label(path: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(path.as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    format!("md:{}", &hex[..12])
}

/// Server-side spawn — bypasses the frontend round-trip. Called from
/// `RunEvent::Opened` and the single-instance argv callback so a freshly
/// arrived path opens its window even when the main webview is dormant
/// (main window is `visible: false` at startup; its webview only loads
/// after `window.show()`).
pub fn spawn_markdown_window<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    path: &std::path::Path,
) -> Result<String, String> {
    let path_str = path.to_string_lossy().to_string();
    let label = window_label(&path_str);
    if let Some(existing) = app.get_webview_window(&label) {
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(label);
    }
    let encoded = urlencoding::encode(&path_str);
    let url = format!("markdown-window.html?path={encoded}");
    tauri::WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title("Markdown")
        .inner_size(1100.0, 820.0)
        .min_inner_size(560.0, 420.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    log::info!("spawn_markdown_window: opened window {label} for {path_str}");
    Ok(label)
}
