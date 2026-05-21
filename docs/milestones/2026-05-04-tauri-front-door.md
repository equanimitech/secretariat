# Milestone — Tauri shell becomes the front door

Date: 2026-05-04. Triggered by Marcelo's first onboarding (see
`docs/audits/2026-05-04-onboarding-ux.md` and `docs/pain/`). Distribution

- update + notification gaps converged to one answer: stop building
  those by hand, use the Tauri shell that's already scaffolded.

## What changes

| Surface           | Before                      | After                                         |
| ----------------- | --------------------------- | --------------------------------------------- |
| `Secretariat.app` | scaffolded, unused          | **principal-facing front door**               |
| `sec` CLI         | the front door              | power-user + scripting                        |
| `sec-mcp`         | the Claude integration      | unchanged; app registers it on first launch   |
| `sec daemon`      | LaunchAgent                 | background loop **inside** the Tauri app      |
| Distribution      | tarball + `bash install.sh` | signed `.dmg` (drag-and-drop)                 |
| Update            | manual re-install           | Tauri Updater plugin (Ed25519-signed, silent) |
| Notification      | none                        | `tauri-plugin-notification` (macOS native)    |

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

| T2FM blocker                                       | Tauri solution                                                    |
| -------------------------------------------------- | ----------------------------------------------------------------- |
| Install is opaque / Terminal-only                  | Native macOS install dialog (drag-to-Applications)                |
| No auto-update                                     | Tauri Updater plugin (Ed25519-signed, silent download + relaunch) |
| No notifications on inbox arrival                  | `tauri-plugin-notification` (macOS UNUserNotificationCenter)      |
| Daemon orchestration is a separate install step    | App lifecycle owns it                                             |
| MCP `compose` body-drop bug class                  | Native textarea → IPC → Rust function. No serialization gap.      |
| MCP install is a separate step                     | App registers `sec-mcp` into Claude on first launch               |
| "Did anything come in?" requires sync_now MCP tool | Tray badge + push notification                                    |

## Out of scope for this pivot

- Replacing the CLI. Power users + scripts keep `sec`.
- Replacing the MCP. Claude Code users keep `sec-mcp`.
- A full GUI for envelope composition. v1 of the app surfaces enough for
  the "drag to install + claim invite + see notification + stamp + reply"
  loop. Editor power lives in the CLI / a future GUI revision.
- Windows / Linux. Tauri is cross-platform but the front-door pivot ships
  macOS-only first (matches AGENTS.md "Mac-only Day 1").

## The model — review session, not real-time (locked 2026-05-04)

Refined after first pass at slicing. The original plan leaned toward a
chat-like UX (push notifications, real-time delivery, in-app compose
textarea). That's wrong direction. The tagline locks the model:

> _"Async generative communication for professionals, stamped by humans."_

Three pillars:

- **Drafting is async and non-blocking.** The AI assistant (Claude Code,
  ChatGPT, etc.) drafts envelopes throughout the day; nothing waits on
  the principal. Drafts queue in the outbox.
- **Stamping is principal-initiated, batched.** The principal opens
  Secretariat at a chosen time, runs a _review session_ — sees the
  queue, reads bodies, stamps approved drafts. Stamp = approval = send.
- **No surprises, no notifications.** No push, no banner, no badge that
  pulls attention. Sync happens when the principal initiates it (open
  app, run "sync now"). Background poll exists at the floor (15min) but
  produces no surface — it's just keeping local state warm.

The app's only jobs are:

1. **Onboarding** (one-time) — explain the draft/review/stamp/send flow,
   wire identity, claim invite, register `sec-mcp` into Claude.
2. **Review surface** — inbox view + outbox-queue review session,
   batch-stamp affordance.
3. **Self-update** silently.

What the app explicitly does _not_ do:

- ✗ Notifications — drop entirely. Principal owns when they look.
- ✗ Push-on-enqueue (relay-side) — same reason; replaced by an explicit
  "sync now" button / MCP tool / CLI command.
- ✗ Compose textarea (v0.2) — drafting lives in the AI assistant.
  _Tiny in-app editing/drafting tools may land in v0.3+ as a quality-of-life
  add for principals who want to tweak a draft before stamping; not v0.2._

## Slices (v2 — review-session model)

Vertical slices, smallest first:

1. **Identity slice** — Tauri commands `init_identity()`, `current_identity()`,
   `secretariat_root()`. ✅ shipped 2026-05-04.

2. **Correspondence-invite slice** — invites establish _bilateral
   correspondence_, not platform onboarding. The invite-claim flow is
   semantically "let's be contacts who exchange stamped envelopes," not
   "join Secretariat via my link." This reframe maps directly to the
   book's Agent Contract concept (every correspondence is a bilateral
   contract between two principals).

   Concrete:
   - Register `secretariat://invite/<token>` URL scheme; clicking opens
     the app and claims the invite.
   - Minimal landing page served by the **relay itself** at
     `<relay>/v0/invite/<token>` (HTML view alongside the existing JSON
     via Accept header / `?view=html`): shows inviter DID + purpose, an
     "Open in Secretariat" button (deep link), fallback "Install
     Secretariat" link to the latest GitHub release. Single static HTML,
     no JS framework.
   - **Bidirectional contact-add becomes the defining behavior**: on
     claim, the relay records both DIDs and exposes a notification queue
     the inviter's daemon drains so it auto-adds the claimer as a
     contact. (Was tracked as separate "C. Bidirectional contact"; folds
     in here naturally.)
   - Optional richer relationship metadata may grow over time —
     suggested-name, purpose, an initial bilateral contract document
     signed by both. Out of scope for v0.2; the slice ships with just
     DID + purpose.
   - CLI paste flow stays as fallback for installed power users.

3. **Review surface slice** — Tauri commands `list_inbox()`, `list_outbox_queue()`,
   `read_envelope(path)`, `sync_now()` (explicit pull from registered relays).
   Frontend: a single review window with two tabs (Inbox / Outbox queue).
   Outbox-queue items show body inline + Touch ID stamp button; batch-select
   for stamping multiple at once. Background poll continues at 15min floor
   but renders nothing visible.

4. **Onboarding slice** — multi-step welcome the first time the app opens:
   1. Identity (calls `init_identity`)
   2. Claim invite (paste field; auto-filled if launched via deep link)
   3. Wire MCP into Claude (calls `sec mcp install`)
   4. Explainer: draft/review/stamp/send flow, with diagram or step-by-step.
   5. End state: "Ready to receive your first envelope."

5. **Distribution slice** — `tauri build` + signed/notarized .dmg + Tauri
   Updater bundle. ✅ workflow scaffolded; awaits Apple cert + secret config
   (see release prereqs Things task).

6. **MCP-on-first-launch slice** — folded into onboarding step 3.

### Out of v0.2

- In-app compose / drafting tools (v0.3+ quality-of-life)
- Push-on-enqueue (philosophy mismatch)
- Notifications (philosophy mismatch)
- (was: web landing page punted — pulled back into slice 2 as a relay-served minimal HTML view)
- Boilerplate library (separate pain doc; v0.3+)
- (was: bidirectional contact treated as separate — folded into slice 2 as the defining behavior of a correspondence invite)

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

## Pending external steps (block release, not implementation)

These cannot be automated. Defer until shaping + implementation are done;
land them just before tagging `v0.2.0`. See
`docs/developer/tauri-distribution-setup.md` for the how-to.

### Apple Developer ID certs

- [ ] Generate **Developer ID Application** cert via developer.apple.com or
      Xcode (Settings → Accounts → Manage Certificates → "+")
- [ ] Import `.cer` to keychain; verify with
      `security find-identity -v -p codesigning`
- [ ] Export from keychain to `.p12` with a strong password
- [ ] Add GitHub repo secrets:
  - [ ] `APPLE_CERTIFICATE` (base64 of `.p12`)
  - [ ] `APPLE_CERTIFICATE_PASSWORD`
  - [ ] `APPLE_SIGNING_IDENTITY` (full cert name)
  - [ ] `APPLE_ID` (Apple account email)
  - [ ] `APPLE_PASSWORD` (app-specific password from appleid.apple.com,
        NOT account password)
  - [ ] `APPLE_TEAM_ID` (10-char team ID, top-right of developer.apple.com)

Without these, CI falls back to ad-hoc signing (Gatekeeper warns once
on first open). Pipeline still produces a usable `.dmg`.

### Tauri Updater key backup

- [ ] Back up `.tauri-keys/secretariat-updater` to 1Password / Bitwarden
      under "Secretariat / Tauri updater private key"
- [ ] Add as GitHub repo secret `TAURI_SIGNING_PRIVATE_KEY`
      (base64-encoded; command in dist setup doc)

If the local copy is lost AND the GitHub secret is lost, every shipped
copy of the app stops accepting updates. The pubkey is baked into
released binaries. Recovery = ship a new pubkey via a migration release
(`sec self-update` falls back to manual). High-cost loss; back up.

### Single source of truth

When all six Apple secrets + the Tauri secret exist, this checklist is
complete. CI signs + notarizes + signs updates without further
intervention.
