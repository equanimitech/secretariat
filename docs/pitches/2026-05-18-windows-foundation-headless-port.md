# Windows foundation — headless port (slice 1)

Pitch — 2026-05-18. Source: free-text "shape Windows readiness" (follow-up to inline survey, 2026-05-18 session)

## Boundaries

### Job to be done

As Christophe (Themia, dommage-corporel briefs, Windows-only), I want to install Secretariat on my Windows machine and stamp envelopes with Windows Hello, so that the Themia legal-briefs flow can actually run.

_When:_ Christophe accepts an invite Rafa sends; baseline today is "can't — Secretariat won't build for Windows."

### Appetite

`big` — one week. Override with `--appetite=<size>`.

## Elements

Three primary elements (Tauri shell deferred):

- **CLI + MCP build for Windows.** `cargo build --target x86_64-pc-windows-msvc -p sec -p sec-mcp` succeeds; tests green; CI matrix gains `windows-latest` job producing `.exe` sidecars next to the existing dmg.
- **Named-pipe IPC.** Factor `daemon/src/ipc/server.rs:74` behind an `IpcTransport` trait. Unix-socket and Windows-named-pipe (`\\.\pipe\secretariat-daemon`) impls side-by-side. Same length-prefixed JSON protocol.
- **Claude Desktop config branch.** `claude_desktop_config_path()` at `mcp.rs:164-168` today bails on non-Mac/Linux. Add a `target_os = "windows"` arm pointing at `%APPDATA%\Claude\claude_desktop_config.json`.

Daemon runs foreground on Windows (`sec daemon run` in PowerShell). Windows Hello biometric already ships at `native_biometric.rs:64-83` — no work.

## Risks

### 🐇 Rabbit holes

- **Named-pipe IPC asymmetry.** Tokio's Windows named-pipe API isn't symmetric with `UnixListener` — server pre-allocates pipe instances and handles the "client connects before server posts an instance" race explicitly. Trait abstraction first, then both impls.
- **Hello hardware on Christophe's machine.** If no fingerprint reader / IR camera, Hello falls back to PIN. Verify PIN-stamped is acceptable as substitute-for-wet-signature before promising parity.

### 🏴 Off-sides called

- **Tauri shell port.** 58 macOS guards, quick-pane, tray, vibrancy. Slice 2.
- **Windows Service install.** No `launchctl` analogue. Christophe runs `sec daemon run` foreground in PowerShell.
- **MSI installer + code-signing.** Ship a zip with two `.exe`s; user adds to PATH.

### 🥩 Fat cut

- **Explicit ACL on the key file.** `cfg(unix)` `chmod 0o600` blocks already compile to no-ops on Windows; user-profile inherited ACLs are usually user-only on non-domain machines. Document and ship. Reopen on real exposure.
- **DPAPI-wrapped at-rest keys.** Equivalent posture to macOS today. Out.

### 🧪 Domain knowledge

- **Christophe's machine reality.** Windows 11 Home/Pro? Domain-joined? Hello hardware (fingerprint / IR / PIN-only)? Ask before sinking days into ACL design.
- **French _avocat_ deontology on stamp = signature.** Does ed25519 + Hello satisfy his RGS/eIDAS bar, or is there a qualified-signature requirement we're misreading? In scope to _know_, out of scope to satisfy.

## Pitch

### Problem

Secretariat is Mac-only. Christophe is the second principal in the named bounded-context wedge (Themia legal briefs) and runs Windows exclusively. The product can't validate its second use case until it builds for him.

Earlier survey suggested a multi-week mountain. A closer read flips that: Windows Hello already ships, `launch.rs` already has a `cfg(not(unix))` fallback, file-permission blocks already compile to no-ops on Windows. The real residue is one IPC socket, one CI matrix entry, and one Claude-Desktop-config path branch.

### The bet

One week to ship a **headless Windows install** carrying the full principal flow. Three deliverables:

1. CI green on `windows-latest`: workspace builds, tests pass, `.exe` sidecars published.
2. `IpcTransport` abstraction with Unix-socket and named-pipe impls; integration test round-trips on both OSes.
3. End-to-end smoke: Rafa-Mac ↔ Christophe-Windows envelope, Hello-stamped on the Windows side, verified on the Mac side via the existing relay. Audit logged.

Circuit-breaker on appetite overrun: stop, write up residue, ship what compiles, re-pitch slice 2.

### No-gos

- No Tauri shell port.
- No Windows Service install on Windows.
- No MSI / signed installer / code-signing cert.
- No explicit ACL hardening.
- No Linux build path beyond "compiles in CI."
