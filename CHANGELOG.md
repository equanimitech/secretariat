# Changelog

All notable changes to Secretariat are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased](https://github.com/equanimitech/secretariat/compare/v0.8.1...HEAD)

## [0.8.1](https://github.com/equanimitech/secretariat/compare/v0.7.2...v0.8.1) — 2026-05-21

### Added

* Primary surface: explorer sidebar, content tabs, substrate timeline.

* Cognition SDK: Bun-compiled Agent SDK sidecar behind `CognitionSession`.

* Org-flavored invites (wire protocol extension).

* Org membership via `membership.local.md`; daemon walks `orgs/`.

* Relay per-`(owner, handle)` cursors; multi-queue poll loop; DM-only enumeration source.

* End-to-end channel-sync smoke test on relay.

* Layman folder UI pitch (Tauri shell as folder list).

### Changed

* `ports/cognition` split into routing / launching / session siblings.

### Fixed

* CI: install Bun in `tauri-release` workflow; retry on Bun setup.

## [0.7.2](https://github.com/equanimitech/secretariat/compare/v0.7.1...v0.7.2) — 2026-05-19

### Added

* Relay channel HTTP route — `POST/GET /v0/queue/{did}/{handle}`.

* Always-visible capture row in quick-pane; auto-resize to content.

### Changed

* Rename `channel` → `queue` across relay + client.

* DM rides the channel route (legacy `/v0/inbox` dropped).

### Removed

* Reverted: subscription store (receiver-side sync primitive) — re-landing later.

## [0.7.1](https://github.com/equanimitech/secretariat/compare/v0.7.0...v0.7.1) — 2026-05-19

### Added

* Lexicon: `fileUpdate` for channel-dir sync.

### Changed

* Relay: single queue index keyed by `(owner, handle)`.

### Fixed

* Migrate-v0.7.0: catch v0.6.0-leftover `queues/`; strengthen count gate; reserve `queues/channels` names; empty-dir cleanup.

* Clippy: clear 7 pre-existing errors.

## [0.7.0](https://github.com/equanimitech/secretariat/compare/v0.6.0...v0.7.0) — 2026-05-18

### Changed

* Layout-complete refactor: identity consolidation, contacts, org markdown, `queue_dir` alignment. Peer queues now nest under `channels/`.

## [0.6.0](https://github.com/equanimitech/secretariat/compare/v0.5.6...v0.6.0) — 2026-05-18

### Changed

* Namespace collapse (slice 2): one resolver, `_self` queue-root. Drop legacy `channel:` hints; bare-handle display + grouping.

* MCP tool descriptions, params, prompts aligned with v0.5 handle grammar.

### Added

* Editable user stub for `contract.local.md` body.

### Fixed

* `.DS_Store` sweep before rmdir-on-empty checks during migrate.

## [0.5.6](https://github.com/equanimitech/secretariat/compare/v0.5.5...v0.5.6) — 2026-05-18

### Changed

* Markdown editor UX pass.

## [0.5.5](https://github.com/equanimitech/secretariat/compare/v0.5.4...v0.5.5) — 2026-05-18

### Changed

* Version bump only.

## [0.5.4](https://github.com/equanimitech/secretariat/compare/v0.5.3...v0.5.4) — 2026-05-18

### Fixed

* Hotfix: correct Crepe theme export paths.

## [0.5.3](https://github.com/equanimitech/secretariat/compare/v0.5.2...v0.5.3) — 2026-05-18

### Fixed

* Markdown: swap Crepe theme sheet with `html` dark class.

* Cognition pane: stop hang on legacy IPC commands.

* Deep-link handler ignores non-`secretariat://` URLs.

## [0.5.2](https://github.com/equanimitech/secretariat/compare/v0.5.1...v0.5.2) — 2026-05-17

### Changed

* Biometric gate moved in-process (native), drop Swift helper.

* `QueueHandle` accepts single-segment handles.

### Removed

* Dangling `Cmd::Contact` references; `touchid-prompt` build step from CI.

## [0.5.1](https://github.com/equanimitech/secretariat/compare/v0.5.0...v0.5.1) — 2026-05-17

### Added

* Idea skill: infer routing from repo-local `.secretariat`; confirm non-defaults only.

## [0.5.0](https://github.com/equanimitech/secretariat/compare/v0.4.8...v0.5.0) — 2026-05-17

### Added

* `channel.md` manifest (replaces `.channelDef` JSON, now frontmatter).

### Changed

* Markdown dev loop fixes (Crepe, capability, Vite); computed title propagates to native window chrome.

* MCP vocabulary scrubbed of stale review-surface terms.

### Removed

* Dead `sec contact` command and CLI-local biometric module.

## [0.4.8](https://github.com/equanimitech/secretariat/compare/v0.4.7...v0.4.8) — 2026-05-17

### Added

* Quick-pane `cmdk` launcher with capture fallback.

## [0.4.7](https://github.com/equanimitech/secretariat/compare/v0.4.6...v0.4.7) — 2026-05-17

### Added

* `sec view <path>` — open markdown file in the desktop app.

* Markdown editor surface: Crepe + frontmatter panel + stamp; macOS file association; Tauri commands (`read`/`write`/`open_window`/`take_pending_opens`); atomic file IO with sha256 lock + `PendingOpens` buffer; field-type inference via gray-matter.

* Settings: terminal picker, dev home isolation, in-app updater.

* Deps: `milkdown/crepe`, `gray-matter`, `sha2`/`sha1`/`urlencoding`/`thiserror`, `tauri-plugin-shell`.

## [0.4.6](https://github.com/equanimitech/secretariat/compare/v0.4.5...v0.4.6) — 2026-05-17

### Changed

* Main window simplified: vertical org picker, one button per vault.

## [0.4.5](https://github.com/equanimitech/secretariat/compare/v0.4.4...v0.4.5) — 2026-05-17

### Added

* Per-channel cognition overrides: `launch_command` / `launch_args` / `launch_env`.

## [0.4.4](https://github.com/equanimitech/secretariat/compare/v0.4.3...v0.4.4) — 2026-05-17

### Added

* `sec launch` — open Claude Code in a channel-bound `cwd`.

## [0.4.3](https://github.com/equanimitech/secretariat/compare/v0.4.2...v0.4.3) — 2026-05-17

### Added

* Tray icon; hide-on-launch.

### Fixed

* Titlebar.

## [0.4.2](https://github.com/equanimitech/secretariat/compare/v0.4.1...v0.4.2) — 2026-05-17

### Added

* Background-daemon mode.

### Changed

* MCP blurb alignment.

## [0.4.1](https://github.com/equanimitech/secretariat/compare/v0.4.0...v0.4.1) — 2026-05-14

### Added

* Capture refuses unknown channel handles (existence gate).

## [0.4.0](https://github.com/equanimitech/secretariat/compare/v0.3.0...v0.4.0) — 2026-05-13

### Added

* `preferences.toml` + skill suite + resource cleanup across core/cli/mcp/tauri.

* Contract verbs: get/set for channels and orgs (CLI + MCP).

* Accumulate resolver: org-root → ancestors → leaf.

* `ChannelContract` value object + `contract.md` storage; org-root auto-scaffold on `create_channel`.

* Optional `reply_to: DocHash` on envelope for threading.

* Outbox writers + drainer + watcher per-queue.

* Readers walk the substrate tree; inbound routed via `queue_dir` resolver.

* Uniform `queue_dir` resolver — `Recipient → on-disk path`.

* MCP exposes `daemon_tick` + `daemon_status`.

### Changed

* Contracts split: `<channel-dir>/contract.md` is consumption-only.

* `KeyPaths.inbox` / `KeyPaths.outbox` flat globals removed.

### Fixed

* Flat-handle captures emit under `envelopes/YYYY/MM/DD/`.

### Removed

* Legacy cognition commands from Tauri.

## [0.3.0](https://github.com/equanimitech/secretariat/compare/v0.2.16...v0.3.0) — 2026-05-12

### Added

* Orgs + channels substrate: channel-tree, orgs/channels CRUD.

* Daemon extracted into its own crate; IPC socket; FS-notify outbox watcher.

### Changed

* v0.3 design pass: selective-stamp, channels, owner-as-sequencer.

### Fixed

* Daemon: serialize tick across IPC + poll loop.

## [0.2.16](https://github.com/equanimitech/secretariat/compare/v0.2.15...v0.2.16) — 2026-05-06

### Added

* Verb-first home; contextification substrate (BYOK + Ollama).

## [0.2.15](https://github.com/equanimitech/secretariat/compare/v0.2.14...v0.2.15) — 2026-05-06

### Fixed

* Mount `PreferencesDialog` so Settings actually opens.

## [0.2.14](https://github.com/equanimitech/secretariat/compare/v0.2.13...v0.2.14) — 2026-05-06

### Added

* Settings panes: Paths, Shortcut, Relay, Integrations.

## [0.2.13](https://github.com/equanimitech/secretariat/compare/v0.2.12...v0.2.13) — 2026-05-06

### Fixed

* Stale tool/prompt descriptions.

## [0.2.12](https://github.com/equanimitech/secretariat/compare/v0.2.11...v0.2.12) — 2026-05-06

### Fixed

* Updater dialog actually shows install button.

## [0.2.11](https://github.com/equanimitech/secretariat/compare/v0.2.10...v0.2.11) — 2026-05-06

### Changed

* Trim MCP surface to 8-tool floor — drop `list`/`defer`/`add_contact`; rename invite verbs.

## [0.2.10](https://github.com/equanimitech/secretariat/compare/v0.2.9...v0.2.10) — 2026-05-06

### Changed

* MCP surface: 16 tools → 12 tools + 3 resources.

## [0.2.9](https://github.com/equanimitech/secretariat/compare/v0.2.8...v0.2.9) — 2026-05-06

### Fixed

* Silent-wire fires on app upgrade, not just app move.

## [0.2.8](https://github.com/equanimitech/secretariat/compare/v0.2.7...v0.2.8) — 2026-05-06

### Added

* C-tier MCP prompts, resources, tool annotations.

## [0.2.7](https://github.com/equanimitech/secretariat/compare/v0.2.6...v0.2.7) — 2026-05-06

### Added

* `/idea`, `/pain` MCP prompts.

### Fixed

* MCP tool registration.

## [0.2.6](https://github.com/equanimitech/secretariat/compare/v0.2.5...v0.2.6) — 2026-05-05

### Fixed

* `wire_claude_code` falls back to known paths when `PATH` lacks `claude`.

## [0.2.5](https://github.com/equanimitech/secretariat/compare/v0.2.4...v0.2.5) — 2026-05-05

### Fixed

* Sidecar build script path corrected to `src-tauri/scripts/` (Tauri runs `beforeBuildCommand` from workspace root). Restores sidecar inclusion that silently regressed in 0.2.3.

## [0.2.4](https://github.com/equanimitech/secretariat/compare/v0.2.3...v0.2.4) — 2026-05-05

### Added

* Substrate slice 1a: `QueueHandle`, `Recipient`, `EnvelopeKind`.

* Substrate slice 1b: `Recipient::{Peer, LocalQueue}` + capture primitive.

* Bundle `sec` sidecars; auto-wire MCP + daemon on app launch.

## [0.2.3](https://github.com/equanimitech/secretariat/compare/v0.2.2...v0.2.3) — 2026-05-05

### Changed

* UI strip: title bar window controls + two-button home only.

## [0.2.2](https://github.com/equanimitech/secretariat/compare/v0.2.1...v0.2.2) — 2026-05-05

### Added

* Inbox primitives: defer + archive.

* Menu app-name fix.

* Settings narrowed to Profile only; buttons copy Claude-ready prompts.

### Fixed

* Docs: `TAURI_SIGNING_PRIVATE_KEY` secret takes raw file contents, not base64.

## [0.2.1](https://github.com/equanimitech/secretariat/compare/v0.2.0...v0.2.1) — 2026-05-05

### Added

* First signed + notarized `.dmg` (copy zenborg's proven workflow + sign config + updater permission).

* Two-button home (review-session entry points).

## [0.2.0](https://github.com/equanimitech/secretariat/compare/v0.1.2...v0.2.0) — 2026-05-05

### Added

* First `.dmg` release — Tauri shell becomes the principal-facing front door.

* In-app stamp + send; immediate send after stamp.

* Two-screen onboarding wizard.

* Principal display name (presence, distinct from identity).

* Background sync loop in setup hook (silent, principal-initiated still primary).

* Review-surface commands: `list_inbox`, `list_review_queue`, `read_envelope`.

* Deep-link claim handler: `secretariat://<host>/v0/invite/<token>`.

* Minimal HTML landing page on relay; deep-link scheme registered.

* Bilateral contact-add — defining behavior of correspondence invites.

* CI: release workflow that builds + signs + notarizes Secretariat.app.

### Changed

* Daemon tick extracted to `application::sync_now` (single source of truth).

* Tests use synthetic DIDs only — never embed real principals.

### Fixed

* Bump Rust crate to 2.11; switch `beforeDevCommand` to `pnpm`.

* pnpm-only invocations + native binding deps for `darwin-arm64`.

## [0.1.2](https://github.com/equanimitech/secretariat/compare/v0.1.1...v0.1.2) — 2026-05-04

### Added

* T2FM round 1: install auto-onboards; compose accepts body; faster polls.

### Fixed

* Relay: drop `VOLUME` directive (Railway rejects it; mount is in `railway.json`).

* MCP: drop project-scope `.mcp.json`; user-scope via `sec mcp install` is more reliable.

## [0.1.1](https://github.com/equanimitech/secretariat/compare/v0.1.0...v0.1.1) — 2026-05-04

### Added

* LaunchAgent install; `init` / `daemon` MCP tools — MCP-driven onboarding.

## [0.1.0](https://github.com/equanimitech/secretariat/releases/tag/v0.1.0) — 2026-05-04

### Added

* Initial release: v0 correspondence loop — relay + crypto + daemon + CLI.

* Day 1: embed-stamp CLI + DDD core; `did:key` and `did:web` identity.

* Lexicons under `tech.equanimi.secretariat.*` namespace.

* Secretariat MCP server (`rmcp` 0.8, stdio).

* One-shot `invite` primitive across relay + application + CLI + MCP.

* Persistent volume + `sec mcp install` for one-command setup.

* GitHub Actions release workflow + install script for binary distribution.

* Architecture and orchestration docs.

### Fixed

* Relay: read `PORT` env directly in Rust; drop shell `startCommand`.

* Relay: bump Dockerfile rust 1.85 → 1.90 (`icu_*` needs ≥1.86).

* Relay: switch runtime base from `distroless/cc` to `debian:bookworm-slim`.

