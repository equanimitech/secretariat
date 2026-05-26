# Changelog

All notable changes to Secretariat are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased](https://github.com/equanimitech/secretariat/compare/v0.11.6...HEAD)

Titlebar regression follow-up to v0.11.4's Things-3 integration. The
`Overlay` style left the OS traffic lights off-screen on current Tauri
2.10.3 (default position drifted out of the 32px React row), and the
macOS branch's `justify-between` flex layout collapsed the Settings
button to the left because `TitleBarLeftActions` returns `null` — a
single flex child can't be spread.

### Fixed

- **Traffic lights anchored in the React row.** Added
  `trafficLightPosition: {x: 16, y: 12}` to the main window in
  `tauri.conf.json` so the OS draws the close/minimize/fullscreen
  controls at a known location inside our 32px integrated bar. Without
  the explicit position, current Tauri left them off-canvas under the
  `Overlay` style.
- **Settings button back on the right.** `TitleBar.tsx`'s macOS branch
  swapped `justify-between` (broken with a `null` left child) for
  `ml-auto` on the right wrapper. Settings now anchors at the row's
  trailing edge as intended.

## [0.11.6](https://github.com/equanimitech/secretariat/compare/v0.11.5...v0.11.6) — 2026-05-26

Drop author-declared envelope attention hints. Two fields on every
envelope (`depth ∈ {gross, subtle}` and `urgency ∈ {now, soon, whenever}`)
claimed routing authority they never had — the recipient's
`contract.local.md` cadence + the envelope's `queue_handle` + `kind`
are the routing inputs. The lexicon itself flagged urgency as
_"inflationary by nature; the recipient's per-channel
contract.local.md cadence governs whether it surfaces inline or queues
for review,"_ so we shipped a required field whose interpretation was
"ignore." Cut both. See pitch
`docs/pitches/2026-05-21-drop-envelope-depth-urgency.md`.

In parallel: a defense-in-depth slice on capture frontmatter — the
single-frontmatter invariant. `sec capture` (and the MCP `capture`
tool) now lift any leading frontmatter the caller smuggled in,
reject the three reserved cryptographic keys (`$envelope`,
`$signature`, `$attestation`) outright, and preserve the rest through
the envelope-write. Prevents the Milkdown-autosave / double-`---`
corruption that broke loads on round-trip. See pitch
`docs/pitches/2026-05-21-stamp-comprehension-gate.md`.

### Removed

- **Envelope `depth` and `urgency` fields, end-to-end.** Lexicon
  (`tech.equanimi.secretariat.envelope` no longer requires or accepts
  them), domain (`EnvelopeDepth` / `EnvelopeUrgency` enums deleted,
  `Envelope` / `EnvelopeBuilder` / `EnvelopeWire` no longer carry the
  fields), application use cases (`ComposeRequest` shrinks), CLI
  (`sec compose --depth` / `--urgency` removed), MCP tool surface
  (`compose` tool params + `parse_depth` / `parse_urgency` helpers
  gone), MCP prompt language (`compose.md` / `stamp.md` no longer
  reference the hints), TS bindings + UI (`FrontmatterField` stops
  rendering the rows). Receiver-side parsers stay tolerant — legacy
  envelopes on disk that still carry the keys parse cleanly; the
  fields are silently ignored. No vault migration ships.

### Added

- **Single-frontmatter invariant on capture.** New
  `lift_leading_frontmatter` pass in `capture_to_queue` parses any
  caller-supplied leading `---...---` block, errors on reserved
  cryptographic keys, and merges the rest into the canonical
  frontmatter the substrate writes. Parser gains a `extra:
BTreeMap<String, serde_yaml::Value>` field that flows through
  `parse_document` → `embed_frontmatter_with_extra`. New
  `RESERVED_FRONTMATTER_KEYS` constant + `CaptureError::
ReservedFrontmatterInBody` variant.
- **TS-side double-frontmatter merge.** `parseMarkdown` now loops on
  adjacent `---...---` blocks, merging the first occurrence of each
  key into a single frontmatter object — defends against the
  Milkdown-autosave path that would otherwise rewrite `_` as `\_`,
  `- ` as `* `, and `---` as `***`, bricking later YAML loads.

### Changed

- **`channel_contract.rs` doc comments.** Drop `depth_filter` /
  `urgency_filter` references from the anticipated-fields list; the
  receiver-side contract composes from cadence + handle-tree only.

## [0.11.5](https://github.com/equanimitech/secretariat/compare/v0.11.4...v0.11.5) — 2026-05-26

Updater unblock. The two-workflow release pipeline (`release.yml` for CLI
tarballs + `tauri-release.yml` for the .dmg + `latest.json`) raced two
`release_id`s at the same tag — `softprops/action-gh-release` couldn't
see the Tauri draft, so it created a competing non-draft release; the
Tauri draft carrying `latest.json` sat orphaned and the in-app updater
started returning _"Could not fetch a valid release JSON from the
remote."_ for v0.11.2, .3, and .4.

### Fixed

- **Single `release_id` per tag.** Merged the two workflows into one
  `release.yml`. A `create-release` job finds-or-creates the draft;
  CLI tarball jobs upload via `gh release upload --clobber` (works
  against drafts, unlike `softprops`); the Tauri build job uploads via
  `tauri-action`'s `releaseId`. No two writers ever race at the same
  tag.
- **Atomic publish.** `publish-release` runs an asset-completeness
  gate (`latest.json` + both tarballs + `.dmg` + `.app.tar.gz` + `.sig`)
  and only flips the draft if every expected file is present. Mid-flight
  failure leaves the draft in place for recovery instead of
  half-publishing a "Latest" release with missing updater metadata.
- **Idempotent re-runs.** find-or-create on the draft; `--clobber` on
  CLI uploads; `tauri-action` overwrites by `releaseId`; publish is a
  single PATCH. Re-running failed jobs picks up where it left off
  without duplicating releases.

## [0.11.4](https://github.com/equanimitech/secretariat/compare/v0.11.3...v0.11.4) — 2026-05-26

Things-3-style integrated title bar on macOS. The previous shell drew both
a native title bar ("Secretariat" at the top) and a custom React title
bar below it (also "Secretariat", centered) — the React bar was designed
to replace the native one but `decorations: true` left the native chrome
in place, so users saw two of everything.

### Changed

- **Main window title bar is now `Overlay` with hidden title.** Set
  `titleBarStyle: "Overlay"` and `hiddenTitle: true` in `tauri.conf.json`
  on the main window only. The OS now inlays the traffic lights at the
  top-left of the content area; the native title text is hidden.
  Markdown viewer windows keep their default native title bar — the
  filename is useful context in a doc-viewer context.
- **Custom React title bar drops the duplicated chrome on macOS.**
  `TitleBar.tsx`'s macOS branch no longer renders `MacOSWindowControls`
  (the native overlay provides traffic lights) or `TitleBarTitle` (no
  title text — the row is a drag region with right-aligned action
  buttons). Left padding (`pl-20`) clears the native traffic lights.
  Windows + Linux branches unchanged. `MacOSWindowControls.tsx` retained
  as dead code for now — easier to delete in a later sweep with other
  titlebar cleanup than to chase orphan refs piecemeal.

## [0.11.3](https://github.com/equanimitech/secretariat/compare/v0.11.2...v0.11.3) — 2026-05-26

Drops the runtime activation-policy flip entirely. v0.11.1 introduced it
to hide the dock icon while the main window was hidden; v0.11.2 added a
100 ms runloop-tick workaround for the dock icon not refreshing on the
Accessory → Regular transition. The flip itself was the wrong choice —
CleanMyMac (which we benchmarked against) ships **separate binaries**
with each binary holding one fixed activation policy, not a single
process flipping at runtime. Single-process runtime flipping is
workaround territory and fragile around fullscreen, cmd+tab, and Cocoa
event-loop timing.

The Tauri shell now runs as a normal Regular app (dock icon always
visible, cmd+tab always working). Red-X still hides the window instead
of quitting (Slack/Discord shape); cmd+Q kills the shell entirely. The
daemon (`sec daemon serve`) runs as its own launchd-managed process and
is unaffected by Tauri shell lifecycle — same survival model as before,
without the policy gymnastics.

When the tray earns its own bounded context (own state, own update
cadence, beyond show/quit), the path forward is to split the tray into
a separate binary with `Accessory` policy at compile time, matching
CleanMyMac's actual architecture. Not today.

### Changed

- **Tauri shell stays in `Regular` activation policy.** Removed the
  startup `Accessory` set, the runtime flip in `surface_main_window`,
  the 100 ms delay thread, and the policy drop in the close handler.
  `surface_main_window` simplifies to show + restore-state + focus.
  `tauri.conf.json` flips main window `visible: true` so first launch
  shows the window normally (the previous `visible: false` made sense
  only under the Accessory model).

## [0.11.2](https://github.com/equanimitech/secretariat/compare/v0.11.1...v0.11.2) — 2026-05-26

Follow-up to v0.11.1. The activation-policy flip from `accessory` to
`regular` was landing in code but the dock icon never appeared in
practice — a known Cocoa gotcha. NSApp.setActivationPolicy needs a
runloop tick to propagate before NSApp.activate (which Tauri's
`set_focus` invokes) will refresh the dock state. Without the gap,
the policy change takes effect but the dock icon stays missing and
the app is absent from cmd+tab.

### Fixed

- **Dock icon now appears when the main window opens on macOS.**
  `surface_main_window` now sets the activation policy, then sleeps
  100 ms on a background thread, then dispatches the show/focus
  sequence back to the main thread via `run_on_main_thread`. The
  show/focus logic is extracted into `show_and_focus_main`, used
  directly on non-macOS platforms. Reference:
  https://steipete.me/posts/2025/showing-settings-from-macos-menu-bar-items

## [0.11.1](https://github.com/equanimitech/secretariat/compare/v0.11.0...v0.11.1) — 2026-05-26

Polish release. Fixes a long-standing macOS fullscreen-rendering glitch on
the main window, and slims the developer's working `target/` directory
from 16+ GB to ~4–6 GB by tuning the dev compile profile.

### Fixed

- **Main-window fullscreen on macOS.** The Tauri shell launched in
  `NSApplicationActivationPolicy.accessory` (no dock icon, tray-only),
  which is incompatible with `NSWindow` fullscreen — menubar auto-hide,
  green-button animation, and Space behavior all glitched. The policy
  now flips to `regular` when the main window is surfaced (tray menu,
  dock-click reopen, single-instance fallback) and back to `accessory`
  when the window is hidden (red-X close). Mirrors the CleanMyMac
  in-process pattern; the daemon already runs as a separate
  launchd-managed process and is unaffected by `Cmd+Q`. The `Reopen`
  handler collapses to a single call to `surface_main_window`, removing
  the duplicated show/restore/focus path.

### Changed

- **Dev compile profile tuned for disk.** Workspace `Cargo.toml` now sets
  `[profile.dev] debug = "line-tables-only"` and `incremental = false`.
  Backtraces keep file:line; the unbounded ~4 GB incremental snapshot
  cache is gone. Combined with rust-analyzer's own target directory
  (`.vscode/settings.json`: `rust-analyzer.cargo.targetDir = true`),
  real-world `target/debug` lands around 4–6 GB instead of 16+ GB.
  Background: Tauri statically compiles every dep from source
  (wry + tao + objc2 + reqwest + tokio + serde — hundreds of crates),
  and rust-analyzer's `cargo check` shares the same `target/` by
  default, triggering constant cache invalidation.

## [0.11.0](https://github.com/equanimitech/secretariat/compare/v0.10.2...v0.11.0) — 2026-05-26

Substrate-for-Themia release. The biggest architectural slice since the
initial substrate landed: agents become first-class principals-delegates,
envelopes are signed at compose (not just stamped), drafts and sent are
collapsed into one envelope state, DM/peer/contact primitives are removed
in favor of channels-only correspondence, the vault gains a two-root
channel tree (`channels/` for self, `orgs/<alias>/channels/` for orgs),
and the verifier chain gains a manifest cache that binds agent signatures
to authorizing principals across the federation.

### Added

- **Agent VO + `authorized_agents` on `identity.md`** (Move 1A). Principals
  explicitly delegate signing authority to one or more agents (today only
  the `scribe` role; future roles — `auditor`, `scheduler`, `reader` —
  reuse the same record shape). Each agent record: `{did, role, name,
substrate, added_at}`. The identity record is signed by the principal's
  active key on every save so any tamper of the delegation list is
  cryptographically detectable. CLI: `sec agent add/list/remove/rotate`;
  MCP exposes the same verbs. Per-agent signing keys live at
  `<root>/identity/agents/<name>/key` (mode `0600`, mirror of the
  principal-key pattern).

- **Envelope `$signature` block, mandatory at compose** (Move 2). Every
  envelope on the wire now carries a detached ed25519 author signature
  — typically by a scribe's agent key, optionally by the principal for
  manually-composed envelopes. Distinct cryptographic layer from
  `$attestation` (the stamp): authorship vs. principal disposition,
  three-layer trust per AGENTS.md hard rule #4. Lexicon:
  `tech.equanimi.secretariat.signature` carries `{signer, signerRole,
docHash, signedAt, signature}`.

- **`agentManifest` lexicon + emit + ingest** (Move 1C). On-wire
  publication of the principal-private `authorized_agents` snapshot.
  Manifest envelopes carry two independent signatures (both by the
  principal): inner over the manifest's canonical preimage (lets the
  manifest stand alone in a cache), outer envelope-level `$signature`
  over the body (uniform with every other envelope on the wire). Emit
  triggers: `sec invite accept`, `sec agent add/rotate/remove`. Two
  layers verified on ingest; tamper at either layer surfaces as
  `TamperDetected` so receivers quarantine rather than silently fall
  back to a stale view.

- **Manifest cache + verifier hop 3.** Filesystem-backed cache at
  `<root>/agents/manifests/<signer>/<target>.md` stores verified
  manifest envelopes verbatim and is self-defending against on-disk
  tamper (every lookup re-verifies through ingest). Daemon `file_inbound`
  auto-ingests on every received envelope so the cache stays fresh
  without operator action. `verify_document_layered` now consults the
  cache for agent-signed envelopes and returns a new
  `SignatureOutcome::VerifiedAgent { agent, principal, signed_at }`
  variant when the binding resolves; `OkUnverifiedAgent` becomes a
  transitional state, not a terminal one. CLI verify / read and MCP
  verify all pass the cache root through.

- **Channel-governance `requires_stamp` field** (Move 6 scaffold). New
  optional field on `tech.equanimi.secretariat.channelDef` lets channel
  owners declare that only stamped envelopes are policy-conformant for
  that channel (concrete driver: Themia `assemblee_generale`). The
  enforcement-side gate is receiver-side and not yet wired; the lexicon
  field and Rust shape are present so authors can declare the policy now.

- **Tauri cognition-provider selection screen** on first launch. The
  principal picks their cognition substrate (Claude Code today;
  Anthropic API / Ollama / etc. additively) before the first agent is
  granted; the choice is materialized as `sec agent add --substrate
<substrate>`. Closes the architectural gap where `sec init`
  auto-granted an agent without an explicit cognition decision.

- **Layered verify output, three-state per layer.** `sec verify`,
  MCP `verify`, and `sec read`'s tamper warnings all report `{signature,
stamp, counter_stamps}` independently. CLI exit code 2 if either
  layer reports tamper / invalid / unresolvable; otherwise 0. Receivers
  set policy per channel.

- **Markdown editor "Reload from disk"** — toolbar button (refresh icon) and `⌘R` / `Ctrl+R` shortcut in the markdown reader/editor window. Re-fetches the file, refreshes the SHA-256 conflict guard, and remounts the Crepe editor so the rehydrated body actually renders (Crepe only reads `defaultValue` at mount, so a controlled-prop update wouldn't have worked). When there are unsaved edits the debounced autosave hasn't flushed yet, a VS Code–style confirm dialog offers Save & reload / Discard & reload / Cancel; when clean, the reload is silent with a toast. The shortcut listener uses capture-phase so the editor can't swallow `⌘R`, and `preventDefault` blocks the webview's default reload.

- **Envelope archive / unarchive operation** with toggle controls on tabs, explorer context menu, and titlebar.

- **Themia walkthrough integration test** (`crates/core/tests/themia_walkthrough.rs`).
  Christophe-stand-in adds Claude as scribe → emits agentManifest into
  the `themia.pro` org channel → composes a draft PV with the scribe's
  key → Rafa-stand-in receives, ingests manifest, runs layered verify,
  stamps. End-to-end exercise of Moves 1A/1B/1C/2/4/13 against the
  org-channel path (not the self-channel shortcut).

### Changed

- **Two channel-tree roots** (Move 3c, vault restructure). The vault
  layout is now:
  - `<root>/identity.md` + `<root>/identity/` (no more `_self/` wrapper)
  - `<root>/channels/<segs>/` for self-owned channels
  - `<root>/orgs/<alias>/channels/<segs>/` for org-owned channels
    `queue_dir()` is the sole branching point on `is_self(owner)`. The
    asymmetry encodes locality at the root level — self channels are
    topologically distinct from org channels, never silently merged. New
    `sec migrate vault-v0-10-to-v0-11` command moves prior vaults in
    place (tar snapshot → atomic rename → post-count gate); idempotent
    on resume after crash.

- **Handle namespace collapse** (Move 3a). The `channel:` / `inbox:` /
  bare-DID handle prefixes are gone; handles are now bare
  slash-separated slugs (`assemblee_generale`,
  `dommage-corporel/paris-cohort`). The `inbox:default` synthesizer
  that auto-routed DMs is removed. CLI `sec compose --handle` is now
  required (was defaulting to `inbox:default`).

- **DM / peer / contact primitive removed** (Move 3b). Bilateral 1:1
  correspondence is now a channel-with-2-members — same primitive as
  multi-party. `crates/core/src/infrastructure/contact_store.rs`
  (−454 lines), `application/process_correspondence_claims.rs`
  (−252 lines), and `application/send_envelope.rs` (−134 lines)
  removed; the conceptual savings recur every time the substrate
  grows (federation, governance, audit all only know one shape).

- **One envelope state** (Move 4). `_drafts/` and `sent/` subdirectory
  trees are gone; every envelope — draft, federated, received —
  lives at `<queue>/envelopes/YYYY/MM/DD/<rkey>.md`. Draft state is
  identified by the absence of the `delivered:` frontmatter field;
  the daemon writes it on successful federation. Stamping embeds the
  `$attestation` block in place — no rename, no path change.
  `migrate outbox-to-drafts` updated to route everything into the
  unified `envelopes/` tree even on a post-Move-4 vault.

- **`save_identity` requires the principal key by signature**
  (architecture review follow-up A). The sign-on-save invariant moves
  from convention into the type system; migration callers use an
  explicit `save_identity_unsigned_for_migration` escape hatch.

### Fixed

- Latent bug in `save_identity`: canonical preimage was computed
  against the pre-substitution body, then `BUILTIN_BODY` was written
  to disk when the body was empty — so the signed bytes diverged
  from what landed on disk and every subsequent `load_identity_verified`
  failed. Body substitution now happens before preimage computation.

- Latent bug in `ingest_manifest_from_file`: only the YAML block was
  parsed, never the body — so tamper after the closing `---` slipped
  past the outer-signature check. Ingest now uses `parse_document` so
  the body bytes are seen and a non-empty body (against the emit
  contract) surfaces as `TamperDetected`.

- `vault-v0-10-to-v0-11` migrator bailed on `dst.exists()` rather than
  recognizing a resume-after-crash. The move loop now reads the
  `(src_exists, dst_exists)` decision table: skip on resume, bail only
  on genuine ambiguity. Idempotency-on-resume invariant restored.

- `AgentSubstrate::InvalidChars` error message reported `[a-z0-9_-]+`
  but the parser actually accepts `.` for `ollama-llama3.2`-style
  identifiers. Message corrected to `[a-z0-9_.-]+`.

- MCP `SERVER_INSTRUCTIONS` + `StampParams` docstring described the
  pre-Move-3c vault path. Updated to the two-root layout so the
  client doesn't try to locate composed envelopes at the old
  `<root>/<alias-of-to>/channels/...` path.

### Removed

- `contact_store`, `process_correspondence_claims`, `send_envelope`
  modules. Anything that referenced the DM primitive (`inbox:default`,
  bilateral contracts as DMs, peer-aliased channel-tree roots) is
  removed; channels are the substrate's only correspondence shape.

### Lexicons (record-shape changes)

All lexicon edits land in lockstep with the Rust changes per AGENTS.md
hard rule #3.

- `tech.equanimi.secretariat.identity` — adds `authorized_agents[]` +
  `$signature`.
- `tech.equanimi.secretariat.envelope` — `$signature` block becomes
  mandatory on emission (parser tolerates absent for legacy back-compat).
- `tech.equanimi.secretariat.signature` — new lexicon (Move 2).
- `tech.equanimi.secretariat.agentManifest` — new lexicon (Move 1C),
  documents the two-layer signing contract (inner + outer).
- `tech.equanimi.secretariat.channelDef` — adds optional
  `requires_stamp`.

### Notes

- Receivers running v0.10.2 reading v0.11.0 envelopes will see
  signature blocks they don't understand (gracefully ignored as
  unknown frontmatter). v0.11.0 receivers reading v0.10.2 envelopes
  see `SignatureOutcome::None` — informational, not authoritative.
  Upgrade in lockstep with correspondence partners for full layered-
  verify coverage.

- The first time a v0.11.0 vault sees a manifest from a v0.11.0 peer
  it auto-caches; subsequent envelopes from that peer's scribes
  return `VerifiedAgent`. Cold-start vaults (no manifests yet
  ingested) return `OkUnverifiedAgent` until the relevant principals'
  manifests arrive.

## [0.10.2](https://github.com/equanimitech/secretariat/compare/v0.10.1...v0.10.2) — 2026-05-21

### Fixed

- External links in the markdown editor now open in the system browser instead of trapping the user inside the Tauri webview. A capture-phase document listener intercepts clicks on `<a>` tags whose protocol is `http(s)`, `mailto`, or `tel` and dispatches them to the Tauri opener plugin; in-app schemes (`secretariat:`, `file:`, fragment `#…`) are left to their existing handlers. Installed across the main window, markdown reader/editor window, and quick-pane; `opener:default` permission added to the quick-pane capability so the call resolves there too.

- Explorer sidebar polish: opening an org now surfaces its channels directly under the org root instead of forcing the principal to expand a `channels/` middleman folder (lazy-loading auto-chains the `channels/` subdir after a private/org root expands). Leaf channels no longer render a chevron expander in channel-only mode — the caret was misleading because the underlying `channel.md` / `contract.local.md` / `template.md` siblings are filtered out of the projection. Show-all-files mode is unchanged.

### Changed

- Lint debt cleared so `pnpm check:all` passes again in the release pipeline. `eslint-plugin-react-hooks` v6 added `set-state-in-effect` to its recommended preset; refactored `useIsMobile` to lazy-init via `useState(() => ...)`, kept six legitimate one-shot Tauri IPC fetches behind `eslint-disable-next-line` comments with intent rationale. `react-compiler/static-components` refactor of `ExplorerTree`'s icon picker to a `<NodeIcon />` component (was returning a capitalized component reference per render). Prettier 3.7/3.8 default markdown emphasis flip (`*foo*` → `_foo_`) applied repo-wide; `docs/{ideas,pitches,decisions,audits,superpowers}/` ignored where prettier oscillates on nested-list indentation.

## [0.10.2](https://github.com/equanimitech/secretariat/compare/v0.10.1...v0.10.2) — 2026-05-21

### Fixed

- External links in the markdown editor now open in the system browser instead of trapping the user inside the Tauri webview. A capture-phase document listener intercepts clicks on `<a>` tags whose protocol is `http(s)`, `mailto`, or `tel` and dispatches them to the Tauri opener plugin; in-app schemes (`secretariat:`, `file:`, fragment `#…`) are left to their existing handlers. Installed across the main window, markdown reader/editor window, and quick-pane; `opener:default` permission added to the quick-pane capability so the call resolves there too.

- Explorer sidebar polish: opening an org now surfaces its channels directly under the org root instead of forcing the principal to expand a `channels/` middleman folder (lazy-loading auto-chains the `channels/` subdir after a private/org root expands). Leaf channels no longer render a chevron expander in channel-only mode — the caret was misleading because the underlying `channel.md` / `contract.local.md` / `template.md` siblings are filtered out of the projection. Show-all-files mode is unchanged.

### Changed

- Lint debt cleared so `pnpm check:all` passes again in the release pipeline. `eslint-plugin-react-hooks` v6 added `set-state-in-effect` to its recommended preset; refactored `useIsMobile` to lazy-init via `useState(() => ...)`, kept six legitimate one-shot Tauri IPC fetches behind `eslint-disable-next-line` comments with intent rationale. `react-compiler/static-components` refactor of `ExplorerTree`'s icon picker to a `<NodeIcon />` component (was returning a capitalized component reference per render). Prettier 3.7/3.8 default markdown emphasis flip (`*foo*` → `_foo_`) applied repo-wide; `docs/{ideas,pitches,decisions,audits,superpowers}/` ignored where prettier oscillates on nested-list indentation.

## [0.10.1](https://github.com/equanimitech/secretariat/compare/v0.10.0...v0.10.1) — 2026-05-21

### Fixed

- Release-pipeline recovery. v0.9.0 and v0.10.0 both shipped under the wrong version: `src-tauri/tauri.conf.json` had been left at `0.8.1` while every other manifest bumped, so the signed DMG and the updater manifest (`latest.json`) stamped at `0.8.1` instead of the tagged version. Clients on `0.8.1` saw "up to date" and the auto-updater never fired. v0.10.1 ships all eight versioned manifests in lockstep at `0.10.1` so the artifacts and updater report the correct version. No functional changes from v0.10.0.

### Changed

- `scripts/prepare-release.js` now bumps every versioned manifest from a single `VERSION_FILES` list (8 entries: `package.json` + `tauri.conf.json` + all 6 workspace `Cargo.toml`s), asserts post-bump consistency, and aborts the release on any mismatch. Switched `npm install` → `pnpm install`; final lockfile refresh covers the full workspace `Cargo.lock`.

- `.github/workflows/{tauri-,}release.yml` gain a pre-build `Verify version consistency vs tag` step that refuses to build if any manifest disagrees with the pushed tag. Combined with the script's post-bump assertion, this class of drift now stops the line at CI before producing signed binaries.

## [0.10.0](https://github.com/equanimitech/secretariat/compare/v0.9.0...v0.10.0) — 2026-05-21

### Added

- AI auto-population of envelope `title` / `lede` / `summary` at compose and capture time via a new `CognitionAg` port; dispatched on Claude and OpenAI-compatible adapters. Substrate-pluggable per architectural invariant #5; degrades silently when no cognition is configured. Gate: body ≥ 280 chars OR contains a paragraph break, plaintext only.

- Lexicon: optional `agSource` ("human" | "ai") on `tech.equanimi.secretariat.envelope`. Absent means human (back-compat). Receivers MAY surface the distinction.

- CLI `--title` / `--lede` / `--summary` flags on `sec compose` and `sec capture`; same fields on MCP `compose` and `capture` params.

- `sec migrate outbox-to-drafts [--dry-run]` — one-shot migration command. Per-queue tar snapshot under `.archive/migrations/<ts>/`, pre/post `.md` count gate, `fs::rename` only (per envelopes-never-destroyed rule). Idempotent. Handles concurrent writers via atomic renames + path-disjoint writers.

### Changed

- Substrate: per-queue `outbox/` staging directory dropped. New shape: `<queue>/_drafts/<ts>-<hash>.md` for unstamped drafts, `<queue>/envelopes/YYYY/MM/DD/...` for received and stamped-pending-send envelopes (mixed timeline tree), `<queue>/sent/YYYY/MM/DD/...` for post-delivery archive. The stamp ceremony's atomic `_drafts/ → envelopes/` rename IS the wire-send signal; daemon drain is the safety net.

- Transition shims: `drain_outbox` aliases `drain_pending_sends`; `list_outbox_files` aliases `list_draft_files`; `SyncOutcome.outbox_warnings` field name preserved on the Tauri `SyncReport` wire shape. Remove once all v0.8 daemon/CLI callers migrate.

- MCP `compose` and `capture` tool descriptions updated to surface auto-AG behavior; `compose` / `review` / `stamp` prompts updated to the new drafts vocabulary.

### Fixed

- `App.test.tsx > renders title bar with traffic light buttons` — jsdom `ResizeObserver` shim added; `@/lib/bindings` mock expanded to cover the explorer + main-window surface. Pre: 50 pass / 1 fail. Post: 51/51 across 9 files.

## [0.9.0](https://github.com/equanimitech/secretariat/compare/v0.8.1...v0.9.0) — 2026-05-21

### Added

- Explorer sidebar: channel-only filter by default; bottom toggle reveals raw filesystem.

- Right-click rename across the explorer tree, backed by a `rename_path` Tauri command.

- Nested unread counts per channel (and aggregating on parent folders), persisted in `localStorage`; opening a channel marks descendants read.

- Drag-and-drop channels onto another channel/folder via a new `move_path` Tauri command; rejects cross-org moves, cycles, duplicate basenames, and non-channel targets.

- Pinning: right-click → Pin/Unpin; pinned channels surface in a top-of-sidebar strip with org-prefixed labels; shortcut, not relocation.

- Super-channels: every channel — leaf or super — opens a timeline tab. Super-channels aggregate envelopes from every descendant queue. No special "folder channel" primitive; channels are channels, just nested.

- Envelope timeline previews render markdown (headings, lists, emphasis, inline code); 3-line clamp.

- Lexicon: optional `title` / `lede` / `summary` fields on `tech.equanimi.secretariat.envelope` per attentional-granularity (gross → subtle deepening pathway). Backwards-compatible.

- Persistent slim envelope footer hosting the stamp surface — `Stamp` action when unstamped, `Stamped by <name>` pill with popover (stamper, timestamp, sig short-hash, doc-hash) when stamped. Iterates a stamps array so counter-stamps (v0.4+) drop in additively.

- Assistant terminal preference (WezTerm and Alacritty alongside Terminal.app, iTerm2, Ghostty, Claude Desktop); reaches both Launch-Claude call sites (channel header + explorer menu).

### Changed

- Launch-Claude moved from envelope toolbar to channel header — channel-dir is the activation surface, not the envelope.

- Frontmatter panel: `$`-prefixed protocol blocks (`$envelope`, `$attestation`) collapse to labeled cards with key facts; click expands read-only JSON. Other keys remain editable.

- Unread visualization adopts a calm signal: bold label + muted-gray pill, no red, no notification color; active channel never bolds and never shows a badge.

### Fixed

- macOS fullscreen now drops the hardcoded rounded corners and hides the custom traffic-light buttons; observes `onResized` so the geometry assumptions in the custom-titlebar setup no longer leak in fullscreen.

## [0.8.1](https://github.com/equanimitech/secretariat/compare/v0.7.2...v0.8.1) — 2026-05-21

### Added

- Primary surface: explorer sidebar, content tabs, substrate timeline.

- Cognition SDK: Bun-compiled Agent SDK sidecar behind `CognitionSession`.

- Org-flavored invites (wire protocol extension).

- Org membership via `membership.local.md`; daemon walks `orgs/`.

- Relay per-`(owner, handle)` cursors; multi-queue poll loop; DM-only enumeration source.

- End-to-end channel-sync smoke test on relay.

- Layman folder UI pitch (Tauri shell as folder list).

### Changed

- `ports/cognition` split into routing / launching / session siblings.

### Fixed

- CI: install Bun in `tauri-release` workflow; retry on Bun setup.

## [0.7.2](https://github.com/equanimitech/secretariat/compare/v0.7.1...v0.7.2) — 2026-05-19

### Added

- Relay channel HTTP route — `POST/GET /v0/queue/{did}/{handle}`.

- Always-visible capture row in quick-pane; auto-resize to content.

### Changed

- Rename `channel` → `queue` across relay + client.

- DM rides the channel route (legacy `/v0/inbox` dropped).

### Removed

- Reverted: subscription store (receiver-side sync primitive) — re-landing later.

## [0.7.1](https://github.com/equanimitech/secretariat/compare/v0.7.0...v0.7.1) — 2026-05-19

### Added

- Lexicon: `fileUpdate` for channel-dir sync.

### Changed

- Relay: single queue index keyed by `(owner, handle)`.

### Fixed

- Migrate-v0.7.0: catch v0.6.0-leftover `queues/`; strengthen count gate; reserve `queues/channels` names; empty-dir cleanup.

- Clippy: clear 7 pre-existing errors.

## [0.7.0](https://github.com/equanimitech/secretariat/compare/v0.6.0...v0.7.0) — 2026-05-18

### Changed

- Layout-complete refactor: identity consolidation, contacts, org markdown, `queue_dir` alignment. Peer queues now nest under `channels/`.

## [0.6.0](https://github.com/equanimitech/secretariat/compare/v0.5.6...v0.6.0) — 2026-05-18

### Changed

- Namespace collapse (slice 2): one resolver, `_self` queue-root. Drop legacy `channel:` hints; bare-handle display + grouping.

- MCP tool descriptions, params, prompts aligned with v0.5 handle grammar.

### Added

- Editable user stub for `contract.local.md` body.

### Fixed

- `.DS_Store` sweep before rmdir-on-empty checks during migrate.

## [0.5.6](https://github.com/equanimitech/secretariat/compare/v0.5.5...v0.5.6) — 2026-05-18

### Changed

- Markdown editor UX pass.

## [0.5.5](https://github.com/equanimitech/secretariat/compare/v0.5.4...v0.5.5) — 2026-05-18

### Changed

- Version bump only.

## [0.5.4](https://github.com/equanimitech/secretariat/compare/v0.5.3...v0.5.4) — 2026-05-18

### Fixed

- Hotfix: correct Crepe theme export paths.

## [0.5.3](https://github.com/equanimitech/secretariat/compare/v0.5.2...v0.5.3) — 2026-05-18

### Fixed

- Markdown: swap Crepe theme sheet with `html` dark class.

- Cognition pane: stop hang on legacy IPC commands.

- Deep-link handler ignores non-`secretariat://` URLs.

## [0.5.2](https://github.com/equanimitech/secretariat/compare/v0.5.1...v0.5.2) — 2026-05-17

### Changed

- Biometric gate moved in-process (native), drop Swift helper.

- `QueueHandle` accepts single-segment handles.

### Removed

- Dangling `Cmd::Contact` references; `touchid-prompt` build step from CI.

## [0.5.1](https://github.com/equanimitech/secretariat/compare/v0.5.0...v0.5.1) — 2026-05-17

### Added

- Idea skill: infer routing from repo-local `.secretariat`; confirm non-defaults only.

## [0.5.0](https://github.com/equanimitech/secretariat/compare/v0.4.8...v0.5.0) — 2026-05-17

### Added

- `channel.md` manifest (replaces `.channelDef` JSON, now frontmatter).

### Changed

- Markdown dev loop fixes (Crepe, capability, Vite); computed title propagates to native window chrome.

- MCP vocabulary scrubbed of stale review-surface terms.

### Removed

- Dead `sec contact` command and CLI-local biometric module.

## [0.4.8](https://github.com/equanimitech/secretariat/compare/v0.4.7...v0.4.8) — 2026-05-17

### Added

- Quick-pane `cmdk` launcher with capture fallback.

## [0.4.7](https://github.com/equanimitech/secretariat/compare/v0.4.6...v0.4.7) — 2026-05-17

### Added

- `sec view <path>` — open markdown file in the desktop app.

- Markdown editor surface: Crepe + frontmatter panel + stamp; macOS file association; Tauri commands (`read`/`write`/`open_window`/`take_pending_opens`); atomic file IO with sha256 lock + `PendingOpens` buffer; field-type inference via gray-matter.

- Settings: terminal picker, dev home isolation, in-app updater.

- Deps: `milkdown/crepe`, `gray-matter`, `sha2`/`sha1`/`urlencoding`/`thiserror`, `tauri-plugin-shell`.

## [0.4.6](https://github.com/equanimitech/secretariat/compare/v0.4.5...v0.4.6) — 2026-05-17

### Changed

- Main window simplified: vertical org picker, one button per vault.

## [0.4.5](https://github.com/equanimitech/secretariat/compare/v0.4.4...v0.4.5) — 2026-05-17

### Added

- Per-channel cognition overrides: `launch_command` / `launch_args` / `launch_env`.

## [0.4.4](https://github.com/equanimitech/secretariat/compare/v0.4.3...v0.4.4) — 2026-05-17

### Added

- `sec launch` — open Claude Code in a channel-bound `cwd`.

## [0.4.3](https://github.com/equanimitech/secretariat/compare/v0.4.2...v0.4.3) — 2026-05-17

### Added

- Tray icon; hide-on-launch.

### Fixed

- Titlebar.

## [0.4.2](https://github.com/equanimitech/secretariat/compare/v0.4.1...v0.4.2) — 2026-05-17

### Added

- Background-daemon mode.

### Changed

- MCP blurb alignment.

## [0.4.1](https://github.com/equanimitech/secretariat/compare/v0.4.0...v0.4.1) — 2026-05-14

### Added

- Capture refuses unknown channel handles (existence gate).

## [0.4.0](https://github.com/equanimitech/secretariat/compare/v0.3.0...v0.4.0) — 2026-05-13

### Added

- `preferences.toml` + skill suite + resource cleanup across core/cli/mcp/tauri.

- Contract verbs: get/set for channels and orgs (CLI + MCP).

- Accumulate resolver: org-root → ancestors → leaf.

- `ChannelContract` value object + `contract.md` storage; org-root auto-scaffold on `create_channel`.

- Optional `reply_to: DocHash` on envelope for threading.

- Outbox writers + drainer + watcher per-queue.

- Readers walk the substrate tree; inbound routed via `queue_dir` resolver.

- Uniform `queue_dir` resolver — `Recipient → on-disk path`.

- MCP exposes `daemon_tick` + `daemon_status`.

### Changed

- Contracts split: `<channel-dir>/contract.md` is consumption-only.

- `KeyPaths.inbox` / `KeyPaths.outbox` flat globals removed.

### Fixed

- Flat-handle captures emit under `envelopes/YYYY/MM/DD/`.

### Removed

- Legacy cognition commands from Tauri.

## [0.3.0](https://github.com/equanimitech/secretariat/compare/v0.2.16...v0.3.0) — 2026-05-12

### Added

- Orgs + channels substrate: channel-tree, orgs/channels CRUD.

- Daemon extracted into its own crate; IPC socket; FS-notify outbox watcher.

### Changed

- v0.3 design pass: selective-stamp, channels, owner-as-sequencer.

### Fixed

- Daemon: serialize tick across IPC + poll loop.

## [0.2.16](https://github.com/equanimitech/secretariat/compare/v0.2.15...v0.2.16) — 2026-05-06

### Added

- Verb-first home; contextification substrate (BYOK + Ollama).

## [0.2.15](https://github.com/equanimitech/secretariat/compare/v0.2.14...v0.2.15) — 2026-05-06

### Fixed

- Mount `PreferencesDialog` so Settings actually opens.

## [0.2.14](https://github.com/equanimitech/secretariat/compare/v0.2.13...v0.2.14) — 2026-05-06

### Added

- Settings panes: Paths, Shortcut, Relay, Integrations.

## [0.2.13](https://github.com/equanimitech/secretariat/compare/v0.2.12...v0.2.13) — 2026-05-06

### Fixed

- Stale tool/prompt descriptions.

## [0.2.12](https://github.com/equanimitech/secretariat/compare/v0.2.11...v0.2.12) — 2026-05-06

### Fixed

- Updater dialog actually shows install button.

## [0.2.11](https://github.com/equanimitech/secretariat/compare/v0.2.10...v0.2.11) — 2026-05-06

### Changed

- Trim MCP surface to 8-tool floor — drop `list`/`defer`/`add_contact`; rename invite verbs.

## [0.2.10](https://github.com/equanimitech/secretariat/compare/v0.2.9...v0.2.10) — 2026-05-06

### Changed

- MCP surface: 16 tools → 12 tools + 3 resources.

## [0.2.9](https://github.com/equanimitech/secretariat/compare/v0.2.8...v0.2.9) — 2026-05-06

### Fixed

- Silent-wire fires on app upgrade, not just app move.

## [0.2.8](https://github.com/equanimitech/secretariat/compare/v0.2.7...v0.2.8) — 2026-05-06

### Added

- C-tier MCP prompts, resources, tool annotations.

## [0.2.7](https://github.com/equanimitech/secretariat/compare/v0.2.6...v0.2.7) — 2026-05-06

### Added

- `/idea`, `/pain` MCP prompts.

### Fixed

- MCP tool registration.

## [0.2.6](https://github.com/equanimitech/secretariat/compare/v0.2.5...v0.2.6) — 2026-05-05

### Fixed

- `wire_claude_code` falls back to known paths when `PATH` lacks `claude`.

## [0.2.5](https://github.com/equanimitech/secretariat/compare/v0.2.4...v0.2.5) — 2026-05-05

### Fixed

- Sidecar build script path corrected to `src-tauri/scripts/` (Tauri runs `beforeBuildCommand` from workspace root). Restores sidecar inclusion that silently regressed in 0.2.3.

## [0.2.4](https://github.com/equanimitech/secretariat/compare/v0.2.3...v0.2.4) — 2026-05-05

### Added

- Substrate slice 1a: `QueueHandle`, `Recipient`, `EnvelopeKind`.

- Substrate slice 1b: `Recipient::{Peer, LocalQueue}` + capture primitive.

- Bundle `sec` sidecars; auto-wire MCP + daemon on app launch.

## [0.2.3](https://github.com/equanimitech/secretariat/compare/v0.2.2...v0.2.3) — 2026-05-05

### Changed

- UI strip: title bar window controls + two-button home only.

## [0.2.2](https://github.com/equanimitech/secretariat/compare/v0.2.1...v0.2.2) — 2026-05-05

### Added

- Inbox primitives: defer + archive.

- Menu app-name fix.

- Settings narrowed to Profile only; buttons copy Claude-ready prompts.

### Fixed

- Docs: `TAURI_SIGNING_PRIVATE_KEY` secret takes raw file contents, not base64.

## [0.2.1](https://github.com/equanimitech/secretariat/compare/v0.2.0...v0.2.1) — 2026-05-05

### Added

- First signed + notarized `.dmg` (copy zenborg's proven workflow + sign config + updater permission).

- Two-button home (review-session entry points).

## [0.2.0](https://github.com/equanimitech/secretariat/compare/v0.1.2...v0.2.0) — 2026-05-05

### Added

- First `.dmg` release — Tauri shell becomes the principal-facing front door.

- In-app stamp + send; immediate send after stamp.

- Two-screen onboarding wizard.

- Principal display name (presence, distinct from identity).

- Background sync loop in setup hook (silent, principal-initiated still primary).

- Review-surface commands: `list_inbox`, `list_review_queue`, `read_envelope`.

- Deep-link claim handler: `secretariat://<host>/v0/invite/<token>`.

- Minimal HTML landing page on relay; deep-link scheme registered.

- Bilateral contact-add — defining behavior of correspondence invites.

- CI: release workflow that builds + signs + notarizes Secretariat.app.

### Changed

- Daemon tick extracted to `application::sync_now` (single source of truth).

- Tests use synthetic DIDs only — never embed real principals.

### Fixed

- Bump Rust crate to 2.11; switch `beforeDevCommand` to `pnpm`.

- pnpm-only invocations + native binding deps for `darwin-arm64`.

## [0.1.2](https://github.com/equanimitech/secretariat/compare/v0.1.1...v0.1.2) — 2026-05-04

### Added

- T2FM round 1: install auto-onboards; compose accepts body; faster polls.

### Fixed

- Relay: drop `VOLUME` directive (Railway rejects it; mount is in `railway.json`).

- MCP: drop project-scope `.mcp.json`; user-scope via `sec mcp install` is more reliable.

## [0.1.1](https://github.com/equanimitech/secretariat/compare/v0.1.0...v0.1.1) — 2026-05-04

### Added

- LaunchAgent install; `init` / `daemon` MCP tools — MCP-driven onboarding.

## [0.1.0](https://github.com/equanimitech/secretariat/releases/tag/v0.1.0) — 2026-05-04

### Added

- Initial release: v0 correspondence loop — relay + crypto + daemon + CLI.

- Day 1: embed-stamp CLI + DDD core; `did:key` and `did:web` identity.

- Lexicons under `tech.equanimi.secretariat.*` namespace.

- Secretariat MCP server (`rmcp` 0.8, stdio).

- One-shot `invite` primitive across relay + application + CLI + MCP.

- Persistent volume + `sec mcp install` for one-command setup.

- GitHub Actions release workflow + install script for binary distribution.

- Architecture and orchestration docs.

### Fixed

- Relay: read `PORT` env directly in Rust; drop shell `startCommand`.

- Relay: bump Dockerfile rust 1.85 → 1.90 (`icu_*` needs ≥1.86).

- Relay: switch runtime base from `distroless/cc` to `debian:bookworm-slim`.
