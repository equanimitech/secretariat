# Changelog

All notable changes to Secretariat are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.1](https://github.com/equanimitech/secretariat/compare/v0.12.0...v0.12.1) — 2026-06-02

First **published** build of the 0.12 line. v0.12.0 was tagged but never
released — the release pipeline's version-lockstep guard still grepped the
deleted `relay` crate and exited before building, leaving an empty draft and
v0.11.9 as the newest build the updater could see. v0.12.1 ships the 0.12.0
git-native teardown plus the changes below.

### Added

- **Repo registry (`sec repo`).** `sec repo` (enroll / list / unenroll) plus
  the `repo_add` / `repo_list` / `repo_remove` MCP tools — the substrate
  manifest naming which git repos are in the principal's world.

### Fixed

- **Release pipeline.** The version-lockstep guard no longer references the
  deleted `crates/relay/Cargo.toml`; it now covers the seven live manifests
  (package.json, tauri.conf.json, src-tauri Cargo, core, cli, daemon, mcp).
  This is why v0.12.0 never published.

### Changed

- **Docs realigned to the teardown.** AGENTS.md, README, and the developer
  architecture/launch docs now describe the markdown editor + Signet
  stamp/verify/read core over the git-native substrate; obsolete plans and
  ideas targeting the cut correspondence apparatus were archived.

## [0.12.0](https://github.com/equanimitech/secretariat/compare/v0.11.9...v0.12.0) — 2026-06-01

Git-native teardown. The correspondence apparatus is cut; what remains is
the markdown editor over a plain filesystem tree plus the Signet stamp /
verify / read core. The substrate is now repos, not `~/.secretariat`
queues (envelopes already migrated into repo `docs/` in v0.11.x). One PR,
~21k lines removed, all layers — `cargo build`/`test`/`clippy -D warnings`
green, `cargo check -p secretariat` clean. No on-wire record shapes
changed; no lexicon diff.

### Removed

- **Federation column.** `relay/` crate deleted (dropped from workspace
  members; unused axum/tower/tower-http deps removed). Core loses
  `application/{sync,federation,delivery_policy}`; `transport/` slimmed to
  `RelayState`. Daemon loses the serve poll loop, `outbox_watcher`,
  `relay_register`, and the IPC `TICK` path — kept: the macOS LaunchAgent
  surface (install/uninstall/status) + a keepalive `serve`. MCP drops
  `daemon_tick` / `daemon_status`; CLI `daemon` trimmed to the
  launchagent verbs.
- **Channels / orgs / contracts / compose / capture / invite / review.**
  Core application use cases deleted (`compose_envelope`,
  `channel_def_envelope`, `invite_ops`, `accept_org_membership`,
  `contract_ops`, `inbox_actions`, `review_queue`, `capture_ops`,
  `contextify_capture`); `inbox_ops` keeps only the read/decrypt path.
  CLI loses `compose`/`channels`/`orgs`/`invite`/`list`/`capture`. MCP
  `server.rs` reduced to `stamp` / `read` / `verify` / `agent_*` (org,
  channel, contract, compose, capture, invite, archive tools + the
  orgs/compositions resources removed). Tauri drops the matching
  commands + bindings. The keeper stores backing `sec launch`
  (channel_def/binding/contract/org) are retained.
- **Capture skills follow the substrate shift.** The `.claude/skills/`
  set is realigned to git-native: `/review` (legacy `~/.secretariat/`
  queue-triage) is deleted — superseded by `/review-repos`; the personal
  capture skills (`/decision` `/idea` `/pain` `/question` `/log`) move out
  of the product into `~/.claude/skills/` and re-route off the removed
  `compose`/`capture` MCP tools — `/decision` writes `docs/decisions/*.md`
  then stamps in place; `/idea` `/pain` `/question` capture to Things (repo
  `docs/` escape hatch when code-tied); `/log` appends to the personal
  journal. `/share` + `review-repos` keep their stale correspondence
  cross-refs scrubbed. The repo retains only project/dev-tooling skills
  (`check`, `cleanup`, `init`, `change-package-manager`, `review-repos`,
  `share`).
- **Channel/org/relay/capture UI.** `ReviewSurface`, `OrgPicker`,
  `ChannelPicker`, `ChannelTimeline`, the preferences `RelayPane`, and
  the explorer channel machinery (`PinnedChannels`, `useUnreadCounts`,
  pinned/unread/active stores, `envelope-path`, `preview-render`) are
  gone. `ExplorerTree` is now a plain filesystem tree (dirs toggle, `.md`
  opens in a markdown tab, rename + reveal remain). `SessionTabs` is a
  markdown-only tab host; `QuickPaneApp` is a minimal placeholder; the
  deep-link invite-claim handler is removed.

### Changed

- **Decoupled the stamp/verify/read core from the channel `Envelope`.**
  `AttestedDocument` loses its envelope field; markdown parses
  `$envelope` opaquely; `inbox_ops` read/decrypt reconstitutes the
  `Envelope` on demand — so the federation/compose machinery could be
  deleted without touching the crypto core.

## [0.11.9](https://github.com/equanimitech/secretariat/compare/v0.11.8...v0.11.9) — 2026-05-26

Substrate gets live org membership (Slice A'): one invite now grants
ongoing participation — new channels appear on subscribers' sidebars
within the next poll cycle without re-inviting. Compose hardens its
frontmatter handling. Settings auto-registers a relay on add. The
envelope-card timeline learns to lean on the AG hierarchy instead of
showing raw DIDs.

### Added

- **Live org membership via `channelDef` envelopes.** New lexicon field
  `channelDef.tombstoned` (envelope-history-preserving removal, distinct
  from `retiredAt` soft-retire). Domain gains `ScopeIntent`
  (`Org` / `Subtree(handle)` / `Channels`) with wire-string parse +
  serialize; invite signature canonicalization v1 → v2 adds
  `scope_intent`. `sec invite create` learns `--org / --role /
--channels` with `*` / `<handle>` / `h1,h2` scope; `sec invite claim`
  persists membership and runs eager-bootstrap `sync_now` so the
  sidebar populates on first connect. `sec channels create/delete
--org` emit `channelDef` envelopes; MCP gains parity. Topological
  backfill primitive (`sec orgs backfill-channel-defs <alias>`) replaces
  the removed `sec migrate`. Daemon Move-5 outbound drain walks
  `envelopes/*.md` under `orgs/*/channels/**` each tick, POSTs
  undelivered envelopes to the membership-declared relay, writes
  `delivered: <seq>` in place; self-owned envelopes mark
  `delivered: local`.
- **Receiver-side signer gate on ingest.** `envelope.$signature.signer`
  must match the expected org-owner DID — without this, any relay-
  registered party could mint phantom channels on every subscriber's
  vault. Name + description sanitised at ingest (control chars and
  bidi/zero-width Unicode stripped, length-capped 80 / 500) as
  defense-in-depth against prompt-injection payloads riding into AI
  surfaces.
- **Tombstone replay/forgery gate.** A tombstoned `channelDef` envelope
  is honoured only when its `createdAt` is at least as new as the local
  channel's `created_at`. The signer-DID gate already blocks forgery
  by non-owners; this closes replay of a captured genuine tombstone
  against a since-recreated channel under the same handle. Rejections
  surface as `[sync] tombstone REJECTED …` and leave the local
  manifest in place.
- **Auto-register relay on settings add.** Adding a relay in the
  preferences pane registers the principal with that relay's roster
  automatically — no separate ceremony.
- **Envelope-preview backend surfaces tags + human sender name.**
  `EnvelopePreview` gained `from_name` (resolved against `identity.md`
  display-name + authorized agents + `orgs/*/org.md`) and `tags`
  (lifted from root frontmatter). Callers fall back to a shortened DID
  when no name record matches.

### Changed

- **Envelope cards re-shaped around the AG hierarchy.** Channel-tab
  timeline now caps card width at `max-w-3xl` and centers the column.
  Sender renders as a human name (DID only when unknown). Stamped /
  unstamped status is conveyed quietly — stamped cards carry a
  stronger border + `bg-card` + subtle shadow, unstamped cards sit
  flatter on the page; the loud `stamped`/`unstamped` pills are gone,
  replaced by a single inline `BadgeCheck` glyph on stamped rows.
  Unread envelopes carry a small sky dot in the meta row (driven by
  the existing `unreadStore`). Free-form `tags:` render as filled
  chips; the envelope `source` keeps a dashed-outline chip on the
  same row.
- **`BadgeCheck` is now the standard stamping glyph.** Envelope-footer
  `Stamp` button + stamped-pill both replaced their `Stamp` / `Check`
  lucide icons with `BadgeCheck` for consistency with the timeline.

### Fixed

- **Compose lifts caller body's leading frontmatter; rejects reserved
  keys.** Capture/compose paths that accept a markdown body now extract
  any leading frontmatter block and merge it into the envelope's own
  frontmatter, rather than letting it sit inside the body where parsers
  miss it. Reserved keys (`$envelope`, `$signature`, `$attestation`)
  are refused outright.

## [0.11.8](https://github.com/equanimitech/secretariat/compare/v0.11.7...v0.11.8) — 2026-05-26

Revert v0.11.4's Things-3 integrated chrome. `titleBarStyle: "Overlay" +
hiddenTitle: true` left the main window with no traffic lights (close,
minimize, fullscreen all unreachable), and `trafficLightPosition` in
v0.11.7 didn't bring them back on current Tauri 2.10.3 / macOS Sonoma
— the right move is to drop the experiment and live with the cosmetic
"two Secretariat" headers until a proper integrated-chrome path lands.

In parallel: the markdown surface used to drop its `MarkdownTitlebar`
row when rendered inside a session tab (`embedded={true}`), leaving the
reload / reveal-in-Finder / frontmatter-sidebar / archive actions
unreachable from the tab view. The tab strip names the document; it
doesn't carry those actions. Render the titlebar in both modes.

### Reverted

- **Main window back to default `Visible` title bar.** Removed
  `titleBarStyle`, `hiddenTitle`, and `trafficLightPosition` from
  `tauri.conf.json` so macOS draws its standard chrome again — traffic
  lights work, fullscreen works, the green button means what it says.
- **`TitleBar.tsx` macOS branch restored.** Re-renders
  `MacOSWindowControls` (custom traffic lights, hidden in native
  fullscreen) and `TitleBarTitle` alongside the right-side action
  cluster. The duplicated chrome is back (native bar + React row) but
  every control is reachable.

### Fixed

- **`MarkdownTitlebar` renders in tab view.** Dropped the
  `!embedded &&` gate in `MarkdownWindow.tsx`. The header (title +
  saving indicator + reload / reveal / sidebar / archive buttons)
  now appears whether the markdown file is opened in its own window
  or inside a session tab.

## [0.11.7](https://github.com/equanimitech/secretariat/compare/v0.11.6...v0.11.7) — 2026-05-26

Failed attempt to rescue v0.11.4's integrated chrome. Reverted in
v0.11.8 — see that entry.

### Fixed

- **Traffic lights anchored in the React row.** Added
  `trafficLightPosition: {x: 16, y: 12}` to the main window in
  `tauri.conf.json` so the OS would draw the close/minimize/fullscreen
  controls at a known location inside the 32px integrated bar. Did
  not produce the intended effect on Tauri 2.10.3.
- **Settings button back on the right.** `TitleBar.tsx`'s macOS branch
  swapped `justify-between` (broken with a `null` left child) for
  `ml-auto` on the right wrapper.

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
