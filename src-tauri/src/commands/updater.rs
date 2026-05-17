//! App self-update commands wrapping `tauri-plugin-updater`. Surfaced
//! by the Settings → About pane so the principal can check + install
//! without leaving the app.

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct UpdateInfo {
    /// Version available on the update server.
    pub version: String,
    /// Version currently running.
    pub current_version: String,
    /// Release notes (markdown), if the manifest carries them.
    pub notes: Option<String>,
    /// Release date as RFC3339 string, if present.
    pub date: Option<String>,
}

/// Check the configured update endpoint. Returns `None` when the app is
/// already on the latest version. Errors propagate as user-facing strings.
#[tauri::command]
#[specta::specta]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let updater = app
        .updater()
        .map_err(|e| format!("updater init: {e}"))?;
    let current_version = app.package_info().version.to_string();
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateInfo {
            version: update.version.clone(),
            current_version,
            notes: update.body.clone(),
            date: update.date.map(|d| d.to_string()),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(format!("checking for update: {e}")),
    }
}

/// Download + install the available update, then restart the app. No-op
/// (errors) when no update is pending — call `check_for_update` first.
#[tauri::command]
#[specta::specta]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|e| format!("updater init: {e}"))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("checking for update: {e}"))?
        .ok_or_else(|| "no update available".to_string())?;

    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|e| format!("installing update: {e}"))?;

    app.restart();
}
