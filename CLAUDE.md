# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Read first

@AGENTS.md

`AGENTS.md` is the authoritative project instruction set — hard rules, architectural
invariants, `agent`/`scribe` vocabulary, bounded context, and out-of-scope list. Everything
below is the mechanical layer (commands, layout) that complements it.

## What this is

Secretariat is a cryptographically attested markdown editor. The principal (human) **stamps**;
Claude (the scribe) composes, edits, and _proposes_ stamps — never stamps unattended. Documents
are markdown in git repos; identity + signing key live under `~/.secretariat/`. macOS-only today.

## Commands

Use **pnpm**, never npm/yarn (the `package.json` scripts say `npm run` internally — ignore that).

```bash
pnpm install
pnpm tauri:dev            # full app (builds sidecars first via beforeBuildCommand)
pnpm dev                  # vite only, no Tauri shell

pnpm check:all            # typecheck + eslint + ast-grep + prettier + rust fmt/clippy + both test suites
pnpm fix:all              # autofix pass
pnpm typecheck | lint | format

pnpm test:run             # vitest once
pnpm test                 # vitest watch
pnpm test:run src/lib/foo.test.ts   # single file
pnpm test:run -- -t "name"          # single test by name
```

Rust — the `pnpm rust:*` scripts `cd src-tauri` first, so they only cover that crate. For the
workspace (which is what the quality gate means), run cargo directly:

```bash
cargo test --workspace
cargo clippy -- -D warnings
cargo test -p secretariat-core <test_name>   # single test
```

Gotchas:

- A bare `cargo check -p secretariat` on a clean clone fails until you run
  `src-tauri/scripts/build-sidecars.sh` once (Tauri needs `binaries/sec-<triple>` staged).
- For any `sec` call against the **live** identity/keys, use
  `/Applications/Secretariat.app/Contents/MacOS/sec` — never `./target/debug/sec`.

## Architecture

Cargo workspace + a React/Vite frontend. Dependency arrows point **down**; `domain` depends on
nothing and does no IO (no `std::fs`, no `reqwest`, no `Utc::now()` — time and randomness arrive
as parameters).

```
crates/core        domain → ports → application → infrastructure   (the whole model lives here)
crates/cli         binary `sec`      — commands in crates/cli/src/commands/
crates/mcp         binary `sec-mcp`  — tools via #[tool] in crates/mcp/src/server.rs
crates/daemon      macOS LaunchAgent install/status/keepalive
crates/cognition-claude-sdk   TS/Bun sidecar (not a Cargo member)
src-tauri          markdown editor shell; bundles sec + sec-mcp as sidecars
src                React 19 + Tailwind v4 + Radix/shadcn + Milkdown editor + zustand + react-query
lexicons/          AT-proto-shaped record schemas — source of truth for on-wire shape
```

**Parallel surfaces.** A new principal-facing operation ships on all of them in one change: the
application use case in `crates/core/src/application/<verb>_ops.rs`, the CLI command in
`crates/cli/src/commands/<verb>.rs` (registered in `cli/src/main.rs`), the MCP tool in
`crates/mcp/src/server.rs`, plus tests for the use case and the cross-layer contract.

**Record shapes.** Changing any record shape requires the matching `lexicons/` edit in the _same_
commit. A record-shape change without a lexicon diff is a stop-the-line event.

## Repo skills

`.claude/skills/` — `review-repos` (the git-native review walker: derives stamp state per doc,
renders coarse→fine, stamps on consent), plus `check`, `cleanup`, `share`, `init`,
`change-package-manager`. Agents: `cleanup-analyzer`, `userguide-reviewer`.
