---
migrated_from: equanimi.tech/project/secretariat/editor/ideas/20260517T194530Z-2uuniy.md
---
# `.secretariat` repo-pointer file — capture intelligence

Refines the org-config idea from 2026-05-17T19:26 (`channels.toml` aliases + defaults inside the org). Where that one solves the *naming* friction, this one solves the *context inference* friction. When an MCP server or CLI runs from inside a repo, it should already know which org + channel to capture to — no arguments required.

## Two layers, working together

| Layer | Lives | Role |
|---|---|---|
| **Org** | `~/.secretariat/orgs/<alias>/channels.toml` | Canonical alias dictionary + org-wide defaults |
| **Repo** | `<repo>/.secretariat` (file) or `<repo>/.secretariat/config.toml` (folder) | Thin pointer: "I belong to org X, default queue is Y" |

The org owns the *vocabulary* (alias names → handles). The repo owns the *binding* (which org + which alias is the default here).

## Why repo-level

Claude Code's MCP server inherits `cwd` from the session. When the principal launches Claude Code in `~/Developer/equanimitech/secretariat`, the secretariat MCP server runs with that cwd. A `.secretariat` file in the repo root means the MCP server already knows the right org + default channel — capture calls don't need `org` or `queue` arguments.

Mirrors `.envrc` (direnv), `.tool-versions` (asdf), `.editorconfig` — thin pointers in the repo; canonical definitions elsewhere.

## Proposed shape

```toml
# .secretariat (at repo root)
org = "equanimi.tech"
default_queue = "ideas"  # resolves via equanimi.tech's alias table

[aliases]
# Per-repo overrides (rare — most aliases live with the org)
bug = "channel:secretariat:editor:bugs"
```

## Resolver order

For `capture` (CLI + MCP):
1. Explicit `org` + `queue` args → use them verbatim
2. Walk up from `cwd` for `.secretariat` → load org binding + alias table
3. Fall back to global `~/.secretariat/preferences.toml` defaults
4. Error with helpful message if still ambiguous

Walk-up should stop at `$HOME` or at any `.secretariat` boundary. Mirrors how git, direnv, and CLAUDE.md all walk up.

## Boundary: NOT the same as channel `root_path`

Today `<channel-dir>/contract.local.md` carries `root_path: <abs-path>` — that's the **inverse direction** (channel → repo cwd). `sec launch` reads it to set Claude Code's cwd when launching into a channel.

`.secretariat` in the repo is the **forward direction** (repo cwd → org + channel). The two are siblings, not duplicates. A repo can have `.secretariat` (forward) AND be referenced by a channel's `root_path` (inverse) — both true at once.

## Sibling gap: `move_channel` / `move_envelope` MCP tools

Re-flagged from the original idea — today moving envelopes between channels or trees requires shelling into `~/.secretariat/` with `mv`, which Claude Code's auto-mode classifier blocks (correctly — it's substrate-state mutation outside the proper API). Need substrate-native move tools.

## Scope sketch

Per AGENTS.md "every principal-facing primitive ships on both interfaces":

1. **Domain**: `RepoBinding` value object (org alias + default_queue + optional per-repo aliases).
2. **Application**: `repo_binding::resolve(cwd) -> Option<RepoBinding>` (walks up, merges org alias table).
3. **Infrastructure**: TOML loader; walk-up resolver respecting `$HOME` boundary.
4. **CLI**: thread the resolved binding into `sec capture`, `sec compose`, `sec view`, `sec launch` as defaults.
5. **MCP**: same — `capture` and `compose` tools consult resolver when `org`/`queue` omitted.
6. **Tests**: domain (parse + validate), application (resolution order, walk-up stop), infrastructure (TOML round-trip, missing file, malformed file), CLI integration (resolver fires from a nested cwd), MCP integration.

Roughly 1-2 day slice, depending on how much CLI surface needs threading. Probably overlaps with the existing capture/compose paths cleanly — defaults at the application layer, no domain churn.

## Priority

HIGH — this is the friction that prompted today's 6-envelope migration from the personal tree to the org tree. Today we lost time because the MCP capture defaulted to personal scope. A `.secretariat` in the repo root would have made that impossible.

Queue for v0.4 wedge unless we're carving out a small substrate-DX slice before then.
