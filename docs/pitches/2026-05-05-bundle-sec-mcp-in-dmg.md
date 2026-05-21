# Bundle `sec-mcp` in the .dmg — wizard step 4 actually fires

Pitch — 2026-05-05. Source: live conversation 2026-05-05 (principal noticed `mcp__secretariat__*` tools absent in fresh Claude Code session despite v0.2.3 install).

**Hard dependency:** Onboarding wizard pitch (`docs/pitches/2026-05-04-onboarding-wizard.md`) — its screen-1 step 4 (_Best-effort `sec mcp install`_) is the call site this pitch makes work for non-dev principals.

## Boundaries

### Job to be done

When a non-developer principal drags `Secretariat.app` into `/Applications` and opens it for the first time, I want the assistant connection (Claude Code / Claude Desktop MCP wiring) to be live by the time the wizard closes — no Terminal, no `cargo install`, no `claude mcp add` — so that the principal's first instruction to Claude (_"check my outbox"_) actually resolves `mcp__secretariat__list_outbox` instead of hitting a stale-tool error.

Baseline today: `sec mcp install` exists at `crates/cli/src/commands/mcp.rs:1` and the wizard already calls it (`docs/pitches/2026-05-04-onboarding-wizard.md:36`). But the resolver at `crates/cli/src/commands/mcp.rs:108-130` only looks at `$PATH`, `~/.cargo/bin/sec-mcp`, `~/.local/bin/sec-mcp` — none of which exist for a `.dmg` installer. The wizard call silently no-ops; principal sees no error and discovers the gap only when Claude can't see Secretariat tools.

### Appetite

`tiny` — the primitives all exist. This is two config edits + one resolver branch.

## Elements

Three changes, all in existing files.

- **Place:** `src-tauri/tauri.conf.json` — `bundle.resources` (or `bundle.macOS.resources`)
- **Affordance:** include the compiled `sec-mcp` binary in `Secretariat.app/Contents/Resources/sec-mcp` at build time
- **Connection:** the GitHub release workflow already builds the workspace; just copy the `sec-mcp` artifact into the resources directory before `tauri build` runs

- **Place:** `crates/cli/src/commands/mcp.rs` — `resolve_sec_mcp_binary()` (or whatever the helper is named at line ~107)
- **Affordance:** add a fourth fallback — `Secretariat.app/Contents/Resources/sec-mcp` resolved relative to the running Tauri executable
- **Connection:** when the wizard calls `sec mcp install` from inside the Tauri app, the resolver finds the bundled binary and `claude mcp add secretariat <bundled-path>` succeeds

- **Place:** wizard screen 1, step 4 (already shipping per the onboarding pitch)
- **Affordance:** unchanged — _"Best-effort `sec mcp install` (silent)"_
- **Connection:** with the bundle + resolver in place, _best-effort_ becomes _actually-wires_ on first launch

## Risks

### 🐇 Rabbit holes

- **Codesigning the bundled `sec-mcp`.** Anything inside `Contents/Resources/` that's a Mach-O binary needs the same Developer ID signature as the parent app, otherwise Gatekeeper blocks execution. The existing release workflow signs `Secretariat.app` — verify it recurses into resources, or sign `sec-mcp` separately before bundling.
- **Path containing spaces.** `/Applications/Secretariat.app/Contents/Resources/sec-mcp` is fine, but if anyone ever ships an app with a space in its name, `claude mcp add` writes a JSON value that needs the path quoted/escaped. Test on a non-default install location.
- **Claude Code project-scope vs user-scope.** `sec mcp install` writes to user-scope via `claude mcp add`; the comment at `crates/cli/src/commands/mcp.rs:6-8` distinguishes scopes. For DMG-installed Secretariat, user-scope is right (works in any cwd). Confirm no path tries project-scope here.

### 🏴 Off-sides called

- **No "MCP toggle" in Settings.** The onboarding-wizard pitch (line 60) already established this is silent-by-default. Don't relitigate.
- **Don't add a new `--bundled-path` flag to `sec mcp install`.** Resolver fallback is the right surface; a flag exposes plumbing the principal shouldn't see.

### 🥩 Fat cut

- **Auto-removing the MCP entry on uninstall.** Tempting, but macOS `.app` drag-uninstall doesn't run hooks. Stale entries in `~/.claude.json` pointing at a removed `Secretariat.app` are cosmetic — Claude Code shows them as "failed to connect" and the principal can remove them, or a future `sec mcp uninstall` can. Out of scope.
- **Bundling for Windows / Linux.** Mac-only Day 1 per `AGENTS.md`. Windows lands when the GUI does.

### 🧪 Domain knowledge

- **Does Tauri v2's `bundle.resources` support a binary outside `src-tauri/`?** Need to verify the path syntax — likely `"../target/release/sec-mcp"` resolved at bundle time. Read Tauri v2 docs for `tauri.conf.json` → `bundle.resources`.
- **Does `claude mcp add` accept absolute paths to binaries inside `.app` bundles, or does it want a wrapper script?** Pretty sure absolute path works (it's just a stdio child process), but confirm before betting.
- **`AGENTS.md` rule 1 says `pnpm` not `npm`.** Bundle changes to `tauri.conf.json` flow through `pnpm tauri build` — confirm the resource is copied during that build, not only on a CI-only path.

## Pitch

### Problem

Live failure mode, today (v0.2.3 installed, fresh Claude Code session): principal asks Claude to walk the outbox; Claude reports _"no Secretariat MCP server connected this session"_ and AGENTS.md still says _"MCP server — not built yet."_ AGENTS.md is stale (the crate exists at `crates/mcp/` with all 13 tools), but the principal's experience is correct: the tools aren't wired. The crate compiled, the wizard calls `sec mcp install`, the resolver finds nothing, the call no-ops silently, and the principal hits the gap only when they try to use the assistant.

This is the same failure pattern the audit flagged for the daemon (`docs/audits/2026-05-04-onboarding-ux.md` — _"Daemon not auto-started at install time"_): the install drops a binary, the principal expects everything to work, but a downstream wiring step is gated on a Terminal command they're never told about. For the daemon the missing step is `sec daemon install`; for MCP it's `cargo install --path crates/mcp`. Both are dev-machine assumptions leaking into the .dmg path.

### The bet

Three small edits, betting `tiny`:

1. Bundle the `sec-mcp` binary into `Secretariat.app/Contents/Resources/` via `tauri.conf.json` `bundle.resources`. Release workflow copies it from `target/release/` before `tauri build` runs.
2. Add a fourth fallback to the binary resolver in `crates/cli/src/commands/mcp.rs` — check the path relative to the running Tauri executable for `Contents/Resources/sec-mcp`.
3. Verify Gatekeeper / codesigning recurses into the bundled binary; sign separately if it doesn't.

The wizard step that already exists starts working on first launch, end-to-end, for a principal who has only ever seen the .dmg drag-to-Applications motion.

This pays off the same audit thread the onboarding-wizard pitch opened. After this lands, _the install IS the configuration_ — same shape we want for `sec daemon install` next.

### No-gos

- No new UI surface. No "Connect Claude" button. The wizard already covers this; it just needs the binary to exist.
- No Windows / Linux bundling.
- No auto-uninstall hook.
- No project-scope MCP wiring (`.mcp.json` in cwd) — user-scope only for the DMG path.
- No bundling of `sec` (the CLI) — that's a separate question; the principal isn't supposed to need `sec` in Terminal at all post-DMG.
