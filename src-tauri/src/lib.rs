//! Tauri application library entry point.
//!
//! This module serves as the main entry point for the Tauri application.
//! Command implementations are organized in the `commands` module,
//! and shared types are in the `types` module.

mod bindings;
mod cognition;
mod commands;
pub mod markdown;
mod types;
mod utils;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
#[cfg(target_os = "macos")]
use tauri::{Manager, RunEvent, WindowEvent};

// Re-export only what's needed externally
pub use types::DEFAULT_QUICK_PANE_SHORTCUT;

/// Application entry point. Sets up all plugins and initializes the app.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Dev builds default to `~/.secretariat-dev/` so live `tauri:dev`
    // sessions don't scribble in the principal's prod home. Set
    // `SECRETARIAT_HOME` explicitly to override (e.g. to point dev at
    // prod for migration testing).
    #[cfg(debug_assertions)]
    {
        if std::env::var_os("SECRETARIAT_HOME").is_none() {
            if let Some(home) = dirs::home_dir() {
                let dev_home = home.join(".secretariat-dev");
                std::env::set_var("SECRETARIAT_HOME", &dev_home);
            }
        }
    }

    let builder = bindings::generate_bindings();

    // Export TypeScript bindings in debug builds
    #[cfg(debug_assertions)]
    bindings::export_ts_bindings();

    // Build with common plugins
    let mut app_builder = tauri::Builder::default();

    // Pending-opens buffer for RunEvent::Opened — populated before the
    // frontend webview has registered its listener, drained on frontend ready
    // (and on every single-instance reentry).
    app_builder = app_builder.manage(crate::markdown::pending::PendingOpens::default());
    app_builder = app_builder.manage(crate::commands::sessions::SessionState::default());

    // Single instance plugin must be registered FIRST
    // When user tries to open a second instance, focus the existing window instead
    #[cfg(desktop)]
    {
        app_builder = app_builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Tray click is the primary surface gesture; `open -a Secretariat`
            // (Finder/Spotlight/CLI) is the secondary. Either way, route here.
            // When `secretariat path/to/file.md` reenters, argv[1..] are
            // candidate file paths — spawn their markdown windows directly
            // from Rust so we don't depend on the main webview being mounted
            // (it's hidden on startup; the webview only loads on first show).
            log::info!("single-instance callback fired with argv: {args:?}");
            let mut opened_any = false;
            for arg in args.iter().skip(1) {
                let p = std::path::PathBuf::from(arg);
                if !p.exists() {
                    log::warn!("single-instance: arg path does not exist: {arg}");
                    continue;
                }
                if let Err(e) = crate::commands::markdown::spawn_markdown_window(app, &p) {
                    log::warn!("single-instance: spawn_markdown_window failed: {e}");
                } else {
                    opened_any = true;
                }
            }
            // If no markdown args, fall back to the original surface-main behavior.
            if !opened_any {
                surface_main_window(app);
            }
        }));
    }

    // Window state plugin - saves/restores window position and size
    // Note: quick-pane is denylisted because it's an NSPanel and calling is_maximized() on it crashes
    // See: https://github.com/tauri-apps/plugins-workspace/issues/1546
    #[cfg(desktop)]
    {
        // Save/restore position, size, maximized, decorations, fullscreen — but
        // NOT visibility. The principal opens the window deliberately (tray
        // click, `open -a`); a previously-visible window must not auto-resurrect
        // on next launch, or the docker-daemon contract breaks.
        let state_flags = tauri_plugin_window_state::StateFlags::all()
            - tauri_plugin_window_state::StateFlags::VISIBLE;
        app_builder = app_builder.plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(state_flags)
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
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            log::info!("Application starting up");
            log::debug!(
                "App handle initialized for package: {}",
                app.package_info().name
            );

            // Tray icon — the canonical surface for opening the main window
            // when the principal wants it. Cross-platform: macOS menubar,
            // Windows system tray, Linux StatusNotifierItem (AppIndicator on
            // GNOME). Left-click toggles the window; right-click reveals the
            // menu (also accessible by click on platforms without click-event
            // discrimination).
            #[cfg(desktop)]
            {
                let show_item =
                    MenuItem::with_id(app, "tray-show", "Show Secretariat", true, None::<&str>)?;
                let quit_item =
                    MenuItem::with_id(app, "tray-quit", "Quit Secretariat", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

                TrayIconBuilder::with_id("main-tray")
                    .icon(app.default_window_icon().unwrap().clone())
                    .icon_as_template(true) // macOS template tinting
                    .tooltip("Secretariat")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id().as_ref() {
                        "tray-show" => surface_main_window(app),
                        "tray-quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            if let Err(e) =
                                commands::quick_pane::toggle_quick_pane(tray.app_handle().clone())
                            {
                                log::error!("Tray quick-pane toggle failed: {e}");
                            }
                        }
                    })
                    .build(app)?;
            }

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

            // Spawn the cognition sidecar (Bun-compiled @anthropic-ai/claude-agent-sdk
            // wrapper). Singleton; multiplexes all tab sessions. Managed as
            // `Arc<ClaudeCodeSdkAdapter>` Tauri state.
            if let Err(e) = crate::cognition::claude_code_sdk::install_into(app.handle()) {
                log::error!("cognition sidecar install failed: {e}");
            }

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

            // The background sync/poll loop was removed in the git-native
            // teardown (cut A) along with the federation column. Inbound
            // correspondence over a hosted relay is no longer wired; the
            // git-native substrate grows its own delivery path.

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
                    //
                    // The dock icon stays visible — Slack/Discord shape. Cmd+Q kills the
                    // Tauri shell entirely; daemon survives via its own launchd agent.
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.hide();
                        log::info!("Main window hidden");
                    }
                }
            }

            // macOS: Dock icon clicked — reopen the main window if it was hidden
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => {
                surface_main_window(app_handle);
            }

            // macOS: "Open With" / `open -a Secretariat path/to/file.md` →
            // append paths to PendingOpens and notify the frontend. The event
            // fires before the webview's listener is registered, so the buffer
            // bridges the gap.
            RunEvent::Opened { urls } => {
                log::info!("RunEvent::Opened with {} url(s)", urls.len());
                for url in urls {
                    let Ok(path) = url.to_file_path() else {
                        log::warn!("RunEvent::Opened — url not file-shaped: {url}");
                        continue;
                    };
                    log::info!("RunEvent::Opened — spawning window for {}", path.display());
                    if let Err(e) =
                        crate::commands::markdown::spawn_markdown_window(app_handle, &path)
                    {
                        log::warn!("RunEvent::Opened — spawn_markdown_window failed: {e}");
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

/// Show + unminimize + focus the main window. The Tauri shell runs with
/// `NSApplicationActivationPolicy.regular` throughout (dock icon always
/// visible, normal cmd+tab behavior). Window hide on red-X close gives the
/// "background app" feel without the Cocoa policy-flip gymnastics.
fn surface_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let was_hidden = !window.is_visible().unwrap_or(true);
        let _ = window.show();
        let _ = window.unminimize();

        // The window-state plugin only auto-restores on app startup, not after
        // a hide/show cycle. Without this the window can appear at stale coords.
        if was_hidden {
            use tauri_plugin_window_state::{StateFlags, WindowExt};
            let _ = window.restore_state(StateFlags::all());
        }

        let _ = window.set_focus();
    }
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
