//! Tauri application library entry point.
//!
//! This module serves as the main entry point for the Tauri application.
//! Command implementations are organized in the `commands` module,
//! and shared types are in the `types` module.

mod bindings;
mod commands;
mod types;
mod utils;

use tauri::{Manager, RunEvent, WindowEvent};

// Re-export only what's needed externally
pub use types::DEFAULT_QUICK_PANE_SHORTCUT;

/// Application entry point. Sets up all plugins and initializes the app.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = bindings::generate_bindings();

    // Export TypeScript bindings in debug builds
    #[cfg(debug_assertions)]
    bindings::export_ts_bindings();

    // Build with common plugins
    let mut app_builder = tauri::Builder::default();

    // Single instance plugin must be registered FIRST
    // When user tries to open a second instance, focus the existing window instead
    #[cfg(desktop)]
    {
        app_builder = app_builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
                let _ = window.unminimize();
            }
        }));
    }

    // Window state plugin - saves/restores window position and size
    // Note: quick-pane is denylisted because it's an NSPanel and calling is_maximized() on it crashes
    // See: https://github.com/tauri-apps/plugins-workspace/issues/1546
    #[cfg(desktop)]
    {
        app_builder = app_builder.plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(tauri_plugin_window_state::StateFlags::all())
                .with_denylist(&["quick-pane"])
                .build(),
        );
    }

    // Updater plugin for in-app updates
    #[cfg(desktop)]
    {
        app_builder = app_builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    app_builder = app_builder
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin({
            #[allow(unused_mut)]
            let mut targets = vec![
                // Always log to stdout for development
                tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                // Log to system logs on macOS (appears in Console.app)
                #[cfg(target_os = "macos")]
                tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                    file_name: None,
                }),
            ];
            // Log to webview console — excluded on Linux where the WebKitGTK webview
            // doesn't exist during setup(), causing app.emit() to deadlock on the IPC socket.
            #[cfg(not(target_os = "linux"))]
            targets.push(tauri_plugin_log::Target::new(
                tauri_plugin_log::TargetKind::Webview,
            ));
            tauri_plugin_log::Builder::new()
                // Use Debug level in development, Info in production
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .targets(targets)
                .build()
        });

    // macOS: Add NSPanel plugin for native panel behavior
    #[cfg(target_os = "macos")]
    {
        app_builder = app_builder.plugin(tauri_nspanel::init());
    }

    app_builder
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            log::info!("Application starting up");
            log::debug!(
                "App handle initialized for package: {}",
                app.package_info().name
            );

            // Set up global shortcut plugin (without any shortcuts - we register them separately)
            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::Builder;

                app.handle().plugin(Builder::new().build())?;
            }

            // Load saved preferences and register the quick pane shortcut
            #[cfg(desktop)]
            {
                let saved_shortcut = commands::preferences::load_quick_pane_shortcut(app.handle());
                let shortcut_to_register = saved_shortcut
                    .as_deref()
                    .unwrap_or(DEFAULT_QUICK_PANE_SHORTCUT);

                log::info!("Registering quick pane shortcut: {shortcut_to_register}");
                commands::quick_pane::register_quick_pane_shortcut(
                    app.handle(),
                    shortcut_to_register,
                )?;
            }

            // Create the quick pane window (hidden) - must be done on main thread
            if let Err(e) = commands::quick_pane::init_quick_pane(app.handle()) {
                log::error!("Failed to create quick pane: {e}");
                // Non-fatal: app can still run without quick pane
            }

            // NOTE: Application menu is built from JavaScript for i18n support
            // See src/lib/menu.ts for the menu implementation

            // First-launch plumbing — Tauri app owns wiring the bundled
            // `sec` + `sec-mcp` sidecars into the principal's environment so
            // they never touch Terminal. Both calls are idempotent and gated
            // on marker files: they re-run only when the bundled binary
            // path changes (app moved or upgraded).
            //
            // - MCP wiring → `sec mcp install` (Claude Code + Claude Desktop)
            //   per memory project_mcp_is_primary_interface
            // - Daemon LaunchAgent → `sec daemon install` per the onboarding
            //   audit (`docs/audits/2026-05-04-onboarding-ux.md` —
            //   "Daemon not auto-started at install time")
            tauri::async_runtime::spawn_blocking(|| {
                if let Err(e) = wire_mcp_from_bundled_sec() {
                    log::info!("MCP wiring skipped: {e}");
                }
                if let Err(e) = install_daemon_from_bundled_sec() {
                    log::info!("daemon install skipped: {e}");
                }
            });

            // Background sync — keeps state warm without surfacing notifications.
            // Per the review-session model
            // (memory/feedback_review_session_model.md), the principal-initiated
            // "Sync now" button is the primary affordance; this loop just
            // means the inbox isn't empty when they open the app. Cadence
            // honors `~/.secretariat/cadence.toml` (default 15-min floor,
            // see core::application::delivery_policy).
            tauri::async_runtime::spawn(async {
                use secretariat_core::application::{sync_now, CadenceConfig};
                use secretariat_core::infrastructure::keys::{load_signing_key, KeyPaths};
                use secretariat_core::Did;

                loop {
                    let interval_min = match KeyPaths::discover() {
                        Ok(paths) => CadenceConfig::load_or_default(
                            &paths.root.join("cadence.toml"),
                        )
                        .map(|c| c.poll_interval_minutes)
                        .unwrap_or(15),
                        Err(_) => 15,
                    };
                    tauri::async_runtime::spawn_blocking(move || {
                        std::thread::sleep(std::time::Duration::from_secs(
                            (interval_min as u64).saturating_mul(60),
                        ))
                    })
                    .await
                    .ok();

                    // Skip silently if no identity yet (pre-onboarding) or
                    // if key/DID load fails. Errors don't kill the loop —
                    // try again next tick.
                    let Ok(paths) = KeyPaths::discover() else { continue };
                    let did_file = paths.root.join("did");
                    if !paths.signing_key.exists() || !did_file.exists() {
                        continue;
                    }
                    let Ok(did_str) = std::fs::read_to_string(&did_file) else {
                        continue;
                    };
                    let Ok(did) = Did::parse(did_str.trim()) else {
                        continue;
                    };
                    let Ok(key) = load_signing_key(&paths.signing_key) else {
                        continue;
                    };
                    if let Err(e) = sync_now(&paths, &did, &key).await {
                        log::warn!("background sync failed: {e}");
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(builder.invoke_handler())
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match &event {
            // macOS: Hide the main window instead of quitting so the dock icon can reopen it
            // and the quick-pane shortcut works independently of the main window.
            // On other platforms, the close proceeds normally and the app exits.
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::CloseRequested { api, .. },
                ..
            } if label == "main" => {
                #[cfg(target_os = "macos")]
                {
                    api.prevent_close();

                    // Save window state before hiding
                    use tauri_plugin_window_state::{AppHandleExt, StateFlags};
                    if let Err(e) = app_handle.save_window_state(StateFlags::all()) {
                        log::warn!("Failed to save window state: {e}");
                    }

                    // Hide the window, not the app. app_handle.hide() calls NSApplication.hide()
                    // which sets system-level hidden state — showing an NSPanel while hidden
                    // causes macOS to unhide the entire app, including the main window.
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.hide();
                        log::info!("Main window hidden");
                    }
                }
            }

            // macOS: Dock icon clicked — reopen the main window if it was hidden
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => {
                if let Some(window) = app_handle.get_webview_window("main") {
                    if !window.is_visible().unwrap_or(true) {
                        let _ = window.show();

                        // The window-state plugin only auto-restores on app startup, not after
                        // a hide/show cycle. Without this the window can appear at stale coords.
                        use tauri_plugin_window_state::{StateFlags, WindowExt};
                        let _ = window.restore_state(StateFlags::all());

                        let _ = window.set_focus();
                        log::info!("Main window reopened from dock");
                    }
                }
            }

            // Cleanup on actual exit (Cmd+Q, menu Quit, or window close on non-macOS).
            // RunEvent::Exit fires reliably before the process exits, unlike ExitRequested
            // which doesn't fire for Cmd+Q on macOS (tauri-apps/tauri#9198).
            RunEvent::Exit => {
                log::info!("Application exiting — performing cleanup");

                // Hide the quick-pane panel to prevent crashes during teardown
                #[cfg(target_os = "macos")]
                {
                    use tauri_nspanel::ManagerExt;
                    if let Ok(panel) = app_handle.get_webview_panel("quick-pane") {
                        panel.hide();
                    }
                }

                // Unregister global shortcuts
                #[cfg(desktop)]
                {
                    use tauri_plugin_global_shortcut::GlobalShortcutExt;
                    if let Err(e) = app_handle.global_shortcut().unregister_all() {
                        log::warn!("Failed to unregister global shortcuts: {e}");
                    }
                }

                log::info!("Cleanup complete");
            }

            _ => {}
        });
}

/// Resolve the bundled `sec` and `sec-mcp` sidecars next to the running
/// Tauri exe (e.g. `Secretariat.app/Contents/MacOS/`). Returns `Err` for
/// dev builds where the sidecars aren't staged.
fn bundled_sidecars() -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "current exe has no parent dir".to_string())?;
    let sec = dir.join("sec");
    let sec_mcp = dir.join("sec-mcp");
    if !sec.exists() || !sec_mcp.exists() {
        return Err(format!(
            "bundled sec / sec-mcp not present next to app exe ({}); dev build?",
            dir.display()
        ));
    }
    Ok((sec, sec_mcp))
}

/// Skip if the marker file already records `current_path`. Otherwise run
/// `body` and, on success, write `current_path` to the marker.
fn run_once_per_path(
    marker_name: &str,
    current_path: &str,
    body: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let marker = dirs::home_dir()
        .ok_or_else(|| "no home dir".to_string())?
        .join(".secretariat")
        .join(marker_name);
    if let Ok(prev) = std::fs::read_to_string(&marker) {
        if prev.trim() == current_path {
            return Ok(());
        }
    }
    body()?;
    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&marker, current_path).map_err(|e| format!("writing marker: {e}"))?;
    Ok(())
}

/// Wire the bundled `sec-mcp` into Claude Code / Claude Desktop. Idempotent;
/// re-wires when either the bundled `sec-mcp` path OR the app's version
/// changes (the latter is needed for in-place upgrades — the path stays
/// `/Applications/Secretariat.app/...` across versions, but the binary
/// behind that path is new and may have different MCP capabilities or
/// tool surfaces, so we must re-wire to refresh the registration).
fn wire_mcp_from_bundled_sec() -> Result<(), String> {
    let (sec, sec_mcp) = bundled_sidecars()?;
    let path_str = format!(
        "{}|{}",
        sec_mcp.to_string_lossy(),
        env!("CARGO_PKG_VERSION")
    );
    run_once_per_path(".tauri-mcp-binary-path", &path_str, || {
        log::info!("wiring MCP via bundled sec: {}", sec.display());
        let status = std::process::Command::new(&sec)
            .arg("mcp")
            .arg("install")
            .arg("--binary")
            .arg(&sec_mcp)
            .status()
            .map_err(|e| format!("spawning sec: {e}"))?;
        if !status.success() {
            return Err(format!("`sec mcp install` exited with {status}"));
        }
        log::info!("MCP wired");
        Ok(())
    })
}

/// Install the LaunchAgent that runs the bundled `sec daemon serve` at login
/// and on reboot. Idempotent; re-installs when either the bundled `sec`
/// path OR the app's version changes. Same upgrade-in-place reasoning as
/// `wire_mcp_from_bundled_sec` — path stays constant but the binary
/// behind it changes, and we need launchd to pick up the new code.
fn install_daemon_from_bundled_sec() -> Result<(), String> {
    let (sec, _sec_mcp) = bundled_sidecars()?;
    let path_str = format!("{}|{}", sec.to_string_lossy(), env!("CARGO_PKG_VERSION"));
    run_once_per_path(".tauri-daemon-binary-path", &path_str, || {
        log::info!("installing LaunchAgent via bundled sec: {}", sec.display());
        let status = std::process::Command::new(&sec)
            .arg("daemon")
            .arg("install")
            .status()
            .map_err(|e| format!("spawning sec: {e}"))?;
        if !status.success() {
            return Err(format!("`sec daemon install` exited with {status}"));
        }
        log::info!("LaunchAgent installed");
        Ok(())
    })
}
