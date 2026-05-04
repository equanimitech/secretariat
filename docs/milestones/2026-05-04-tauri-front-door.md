# Milestone — Tauri shell becomes the front door

Date: 2026-05-04. Triggered by Marcelo's first onboarding (see
`docs/audits/2026-05-04-onboarding-ux.md` and `docs/pain/`). Distribution
+ update + notification gaps converged to one answer: stop building
those by hand, use the Tauri shell that's already scaffolded.

## What changes

| Surface | Before | After |
|---|---|---|
| `Secretariat.app` | scaffolded, unused | **principal-facing front door** |
| `sec` CLI | the front door | power-user + scripting |
| `sec-mcp` | the Claude integration | unchanged; app registers it on first launch |
| `sec daemon` | LaunchAgent | background loop **inside** the Tauri app |
| Distribution | tarball + `bash install.sh` | signed `.dmg` (drag-and-drop) |
| Update | manual re-install | Tauri Updater plugin (Ed25519-signed, silent) |
| Notification | none | `tauri-plugin-notification` (macOS native) |

The CLI and MCP **do not go away**. They remain as alternate surfaces for
Claude Code users and shell scripts. But the default install gives the
principal an app icon, not a Terminal command.

## Why now

Three forcing functions converged:

1. **Marcelo's onboarding broke** — install was opaque, daemon needed manual
   install, compose silently dropped the body, no path for the fix to reach
   him without re-downloading. T2FM today: 60+ min in worst case, often
   blocked entirely.
2. **The Tauri scaffold is already mature** — React + i18n + command
   palette + preferences + recovery + notifications + updater plugin
   already wired. We've been carrying the weight without using it.
3. **Apple Developer enrollment is in place** — the only blocker for a real
   signed/notarized .dmg via `tauri build` is generating two certificates
   (Developer ID Application + Installer) and adding them to keychain.

## Architectural fit

Two architectural invariants need to remain true after the pivot:

- **Keys never leave the device** (AGENTS.md inv. 3) — Tauri runs locally;
  signing key stays in `~/.secretariat/key`; Tauri commands call into
  `secretariat-core` directly via `path` dep (already wired in
  `src-tauri/Cargo.toml`).
- **Stamp ceremony is principal-attested** (AGENTS.md rule 4) — Tauri
  doesn't change Touch ID gating. The biometric port stays the same;
  the prompt fires from inside the app's window context.

What changes architecturally:

- Application use cases (`compose_envelope`, `stamp_document`, etc.) get
  exposed as `#[tauri::command]` in addition to the existing CLI/MCP
  invocations. Same functions, three callers.
- Daemon poll loop migrates from `sec daemon serve` (LaunchAgent) to a
  Tauri-managed background task (`tokio::spawn` from `setup` hook).
  LaunchAgent path stays as fallback for headless installs.

## What we get for free from Tauri

| T2FM blocker | Tauri solution |
|---|---|
| Install is opaque / Terminal-only | Native macOS install dialog (drag-to-Applications) |
| No auto-update | Tauri Updater plugin (Ed25519-signed, silent download + relaunch) |
| No notifications on inbox arrival | `tauri-plugin-notification` (macOS UNUserNotificationCenter) |
| Daemon orchestration is a separate install step | App lifecycle owns it |
| MCP `compose` body-drop bug class | Native textarea → IPC → Rust function. No serialization gap. |
| MCP install is a separate step | App registers `sec-mcp` into Claude on first launch |
| "Did anything come in?" requires sync_now MCP tool | Tray badge + push notification |

## Out of scope for this pivot

- Replacing the CLI. Power users + scripts keep `sec`.
- Replacing the MCP. Claude Code users keep `sec-mcp`.
- A full GUI for envelope composition. v1 of the app surfaces enough for
  the "drag to install + claim invite + see notification + stamp + reply"
  loop. Editor power lives in the CLI / a future GUI revision.
- Windows / Linux. Tauri is cross-platform but the front-door pivot ships
  macOS-only first (matches AGENTS.md "Mac-only Day 1").

## Slices

Vertical slices, smallest first:

1. **Identity slice** — Tauri command `init_identity()` + `current_did()`,
   wired through `secretariat-core`. Minimal "Welcome → Generate identity"
   onboarding screen. Replaces `sec init` for the app path.
2. **Invite slice** — `claim_invite(url)` + `create_invite(purpose)`. Paste
   URL into onboarding screen → done.
3. **Inbox slice** — `list_inbox()` + `read_envelope(id)`. Background task
   polls relay every 15min (same cadence as daemon); on new envelope, fires
   notification.
4. **Compose + stamp slice** — `compose_draft(to, body, ...)` writes file;
   `stamp_draft(path)` triggers Touch ID; `send_now(path)` flushes outbox.
   Compose form has a textarea (no body-drop bug class).
5. **Distribution slice** — `tauri build` configured with real updater
   endpoint + Ed25519 signer + notarization in CI. First `Secretariat-0.2.0.dmg`.
6. **MCP-on-first-launch slice** — app calls `sec mcp install` on first
   launch (via process plugin or shelled cmd) so Claude Code integration
   stays one-step.

Slices 1–4 are pure Tauri command additions, no UX rebuild required (the
React shell already has windows + command palette + preferences). Slice 5
is the distribution shift. Slice 6 ties back to the Claude integration.

## Decision log

- **Use the existing src-tauri crate, not a new binary.** It already has
  `secretariat-core` as a path dep + Tauri 2 plugins + React frontend.
  Starting fresh would double the work.
- **Keep the menubar/tray pattern over a main window.** The principal
  spends most time in Claude or their editor; the app is ambient.
  (Quick-pane scaffolding may repurpose to the compose drawer.)
- **Daemon loop runs in-process via `tokio::spawn`** — simpler than
  managing a sidecar; same Rust stack.
- **LaunchAgent stays as headless install option.** Server-class principals
  who don't want a GUI can still `sec daemon install`. The .app is the
  default path, not the only path.

## Success criteria for v0.2.0

- Drag `Secretariat.app` to /Applications → first launch shows onboarding.
- Paste invite URL → connected, contact added, notification permission
  requested.
- Sender stamps an envelope → recipient gets a macOS notification within
  ~30s.
- App auto-updates between 0.2.0 → 0.2.1 with no user action.
- `sec` CLI continues to work end-to-end for power users (regression-free).
- `sec-mcp` continues to work in Claude Code (regression-free).
