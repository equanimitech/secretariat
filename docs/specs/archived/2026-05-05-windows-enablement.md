# Spec — Windows enablement (onboarding cofounder #1)

**Status:** drafted, not started
**Author:** Rafa
**Date:** 2026-05-05
**Appetite:** ~1 week (one engineer, sequential A → B → C)
**Driving use case:** Christophe (Themia cofounder, Windows, avid Claude Code user) starts using Secretariat for bilateral stamped correspondence with Rafa.

## Why

`AGENTS.md` lists "Cross-platform — Mac-only Day 1" as out of scope. The note
is more conservative than the code. Investigation:

- `BiometricGate` is already a clean trait (`crates/core/src/infrastructure/ed25519_signer.rs:18`).
- Touch ID is just a _gate_ in front of file-key signing (key lives at `~/.secretariat/key`, PKCS#8 PEM 0600). The gate has no access to the key.
- `pick_gate()` (`crates/core/src/infrastructure/biometric.rs:42`) dispatches by env + `cfg`. Adding a Windows gate is a one-impl extension, not an architectural change.
- The MCP server is **built** (`crates/mcp/`, `sec-mcp` binary, `sec mcp install` wires Claude Code) — `AGENTS.md` is stale on this point.
- `sec init` already `cfg`-gates its only macOS-specific side effect (`swiftc` for the Touch ID helper). Init on Windows compiles and runs as-is.
- Relay (`https://secretariat.equanimi.tech`) and invite/claim flow already work cross-OS at the protocol level. Tauri deep-link plugin handles `secretariat://` URI registration on Windows.

The blocker is one missing gate impl + a release pipeline that hasn't been
exercised on Windows yet.

## Decisions

- **Gate strategy:** native `WindowsHelloGate` (not passphrase). Keeps the "stamped by humans" contract intact with real OS-level user-presence verification.
- **Distribution:** `.msi` via the existing release pipeline (Tauri action). Forces us to exercise the Windows release flow once.
- **Surface:** CLI + MCP via Claude Code. No Tauri GUI on Windows in this milestone — matches the review-session model and Christophe's profile.
- **Code signing:** unsigned for first cut (SmartScreen warning accepted). Same posture as the unsigned `.dmg` path. Revisit when we have a second Windows user.

## Workstream A — `WindowsHelloGate`

Goal: a `BiometricGate` impl that prompts via `Windows.Security.Credentials.UI.UserConsentVerifier` (the Windows Hello "consent" surface — face / fingerprint / PIN, no WebAuthn ceremony required for local consent).

**Files:**

- **NEW** `crates/core/src/infrastructure/windows_hello.rs`
  - `pub struct WindowsHelloGate;`
  - `impl BiometricGate for WindowsHelloGate { fn prompt(&self, reason: &str) -> Result<(), SignerError> { ... } }`
  - Implementation calls `UserConsentVerifier::CheckAvailabilityAsync()` and `RequestVerificationAsync(reason)` from the `windows` crate.
  - Outcome map:
    - `Verified` → `Ok(())`
    - `DeviceNotPresent | NotConfiguredForUser | DisabledByPolicy` → `SignerError::BiometricUnavailable` (new variant — see below)
    - `RetriesExhausted | Canceled` → `SignerError::BiometricRefused`
    - `DeviceBusy` → retry once with short backoff, then `BiometricRefused`
  - Reason string carries doc headline + hash prefix, exactly like Touch ID (parallel to `AGENTS.md` rule 4).
  - Module gated `#[cfg(target_os = "windows")]`.

- **MODIFY** `crates/core/src/ports/mod.rs` — add `SignerError::BiometricUnavailable`. Distinguishes "user declined / failed Hello" from "no Hello configured" — the latter is actionable (set up Hello in Windows Settings).

- **MODIFY** `crates/core/src/infrastructure/mod.rs` — register module + re-export, mirroring the existing `cfg(target_os = "macos")` pattern for `touchid`:

  ```rust
  #[cfg(target_os = "windows")]
  pub mod windows_hello;
  #[cfg(target_os = "windows")]
  pub use windows_hello::WindowsHelloGate;
  ```

- **MODIFY** `crates/core/src/infrastructure/biometric.rs`
  - Extend `AnyGate`: `#[cfg(target_os = "windows")] WindowsHello(WindowsHelloGate)`.
  - Extend `pick_gate()`:
    - `SECRETARIAT_BIOMETRIC=windows_hello` (explicit) → `WindowsHelloGate`.
    - On Windows, `None | Some("windows_hello")` → `WindowsHelloGate` (default).
    - On Windows, `Some("touchid")` → error: "Touch ID is macOS-only; use windows_hello".
    - macOS branch + `always_allow|always_deny` debug paths unchanged.

- **MODIFY** `crates/core/Cargo.toml`

  ```toml
  [target.'cfg(target_os = "windows")'.dependencies]
  windows = { version = "0.59", features = [
      "Foundation",
      "Security_Credentials_UI",
  ] }
  ```

  Target-gated — no change to mac/linux build.

- **MODIFY** `crates/cli/src/commands/biometric.rs` — add `#[cfg(target_os = "windows")] fn require_windows_hello_available()` paralleling the existing `require_touchid_binary()`. Calls `CheckAvailabilityAsync()` once at init/verify time so failure happens at `sec init`, not on first stamp.

**Tests:**

- Unit: `cargo test --workspace` continues to pass on macOS (new module is `cfg`-gated out). Existing `AlwaysAllowGate` / `AlwaysDenyGate` tests remain the platform-agnostic coverage of the trait contract.
- Windows CI integration test: smoke test under `#[cfg(target_os = "windows")]` constructs `WindowsHelloGate` and asserts `CheckAvailability` returns _some_ result without panicking. Real `Verified` requires user presence — manual end-to-end step.

## Workstream B — Windows release pipeline

Goal: produce a downloadable Windows installer of `sec` + `sec-mcp` (CLI binaries; no Tauri GUI bundle for this milestone).

**Files:**

- **MODIFY** `.github/workflows/tauri-release.yml` (and any `release:prepare` scripts)
  - Extend the build matrix to include `windows-latest`.
  - Build `sec.exe` and `sec-mcp.exe`, packaged as a single `.msi` via `cargo-wix` (preferred) or as a zip + scoop manifest (fallback).
  - Extend `latest.json` with a `windows-x86_64` platform block for future auto-update.

- **MODIFY** `docs/developer/tauri-distribution-setup.md` — add a "Windows" section paralleling the Apple section: `cargo-wix` install, `.msi` packaging, SmartScreen behavior, future EV-cert path, GitHub secrets for Windows code-signing when we add it.

- **NEW** `docs/userguide/install-windows.md` — one-page install guide for the recipient: download `.msi`, accept SmartScreen "More info → Run anyway", `sec init`, `sec mcp install`.

**Verification:** tag `v0.3.0-rc1`; confirm workflow produces `Secretariat-0.3.0-x86_64.msi` and uploads it.

## Workstream C — Onboard Christophe (live run)

Once A + B land, a single end-to-end pass:

1. He downloads `.msi` from GitHub release, runs it. Accepts SmartScreen.
2. `sec init` → fresh `did:key`, key in `%USERPROFILE%\.secretariat\key`, template + attention-envelope scaffolded. Init calls `require_windows_hello_available()` and fails fast with a setup link if Hello isn't configured.
3. `sec mcp install` → `claude mcp add secretariat -s user -- <path>`.
4. Rafa runs `sec invite create` → claim URL.
5. URL shared via Slack (bootstrap channel; no Secretariat-mediated transport between us yet).
6. Christophe runs `sec invite claim <url>` → auto-registers his DID with the relay, auto-adds Rafa to his contacts.
7. Rafa's daemon poll → Christophe shows up in Rafa's contacts.
8. Christophe composes via Claude Code MCP, stamps via Windows Hello, sends. Rafa polls + `sec verify --json` → signature passes against Christophe's `did:key`.

**Step 8 = success condition for this milestone.**

## Workstream D — `AGENTS.md` updates (small, blocking)

Stale claims that will mislead future sessions if left:

- "What's here today" — strike "MCP server — not built yet"; replace with a one-line `sec mcp install` description.
- "Out of scope" — strike "Cross-platform — Mac-only Day 1; Windows when the GUI lands". Replace: "Tauri GUI on Windows (deferred); CLI + MCP on Windows in scope as of v0.3."
- Rule 10 — extend "Tauri v2 only" with the biometric gate matrix (Touch ID on macOS, Windows Hello on Windows, no Linux gate yet).

## Verification (end-to-end)

Rafa's mac:

```bash
cargo test --workspace
cargo clippy -- -D warnings
cargo build --release -p sec -p sec-mcp
```

Christophe's Windows, once `.msi` is published:

```powershell
msiexec /i Secretariat-0.3.0-x86_64.msi
sec init
sec mcp install
sec invite claim secretariat://secretariat.equanimi.tech/invites/<token>
# Compose + stamp via Claude Code MCP. Hello prompts on stamp.
```

Rafa's mac, verify:

```bash
sec daemon poll
sec list --inbox
sec verify --json ~/.secretariat/inbox/<christophe-did>/<env>.md
# expect: { "ok": true, "signer": "did:key:...", ... }
```

## Out of scope (explicit)

- Tauri GUI on Windows — defer until macOS ceremony surface stabilizes.
- Linux gate — Christophe is Windows; leave Linux on `always_deny` (forces explicit dev opt-in).
- Windows code-signing cert — accept SmartScreen for v0.3; revisit when we have a second Windows user.
- Hardware security key (FIDO2 token) gate — orthogonal, future.

## Critical files (quick reference)

- `crates/core/src/infrastructure/ed25519_signer.rs:18` — `BiometricGate` trait
- `crates/core/src/infrastructure/biometric.rs:42` — `pick_gate()` dispatch
- `crates/core/src/infrastructure/mod.rs:14` — `cfg`-gated module registration pattern (touchid)
- `crates/core/src/infrastructure/touchid.rs` — reference impl to mirror in shape
- `crates/cli/src/commands/biometric.rs:19` — CLI helper presence check pattern
- `crates/cli/src/commands/init.rs` — `cfg`-gated init side effect pattern
- `crates/cli/src/commands/stamp.rs:79` — gate consumption site (no change)
- `.github/workflows/tauri-release.yml` — release pipeline to extend
- `docs/developer/tauri-distribution-setup.md` — distribution doc to extend
- `AGENTS.md` — three stale claims to fix
