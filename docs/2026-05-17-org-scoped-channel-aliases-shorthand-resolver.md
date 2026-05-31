---
migrated_from: equanimi.tech/project/secretariat/editor/ideas/20260517T192611Z-zeu3qu.md
---
# Org-scoped channel aliases / shorthand resolver

Pain today: capturing into a deep channel requires the full handle every time. `capture(org="equanimi.tech", queue="channel:secretariat:editor:ideas", body=...)` is verbose. Worse, it's easy to forget the `org` parameter and land in the personal tree (happened today — had to move 6 envelopes back).

What we want: a config inside the org (NOT inside arbitrary git repos) that declares short aliases for frequently-used channels and surfaces them through the MCP and CLI.

## Shape (proposed)

A file at `~/.secretariat/orgs/<alias>/channels.toml` (or extend `contract.local.md` / `.channelDef`) carrying:

```toml
[aliases]
ideas       = "channel:secretariat:editor:ideas"
bugs        = "channel:secretariat:editor:bugs"
dev         = "channel:secretariat:dev"
windows     = "channel:secretariat:windows"

[defaults]
# When org=equanimi.tech is set but queue is omitted, fall through here.
queue = "channel:secretariat:editor:ideas"
```

## Resolver behaviour

- Capture call `queue="ideas"` (no `channel:` prefix) + `org="equanimi.tech"` → resolver expands to `channel:secretariat:editor:ideas` inside that org.
- Capture call `queue="ideas"` + no org → error with a list of orgs that have an `ideas` alias, ask which one.
- CLI `sec capture ideas` (positional) — same expansion. Or `sec capture --to ideas`.

## Why not in repos

User explicitly course-corrected: configs live within the org (substrate tree), not within git repos. Repos and orgs are orthogonal:

- A git repo can map to a channel via `<channel-dir>/contract.local.md`'s `root_path` (already shipping per AGENTS.md), but the channel still lives in the org.
- Repo-level config would invert this — repos referencing orgs. Wrong direction. The org is the canonical home; aliases live with the entity that owns the namespace.

## Sibling gap: no `move_channel` MCP tool

Today moving envelopes between channels (or between personal-tree and org-tree) means shelling into `~/.secretariat/` with `mv` — blocked by Claude Code's auto-mode classifier as substrate-state mutation outside the proper API. We need:

- `move_channel(handle, to_org=...)` — moves the entire channel tree (including `.channelDef`) into a different org.
- `move_envelope(file_path, to_handle=...)` — single-envelope move between channels.

Both should be filesystem-authoritative ops with no envelope mutation (preserve hashes, signatures, timestamps).

## Priority

Aliases: HIGH — friction every single capture, will compound as channel count grows past ~10.
Move tools: MEDIUM — rare op today, but blocking moments like the 2026-05-17 personal-tree-to-org migration that prompted this idea.

Both deferred from the markdown-reader work. File under `secretariat:dev` or a new `secretariat:cli` channel when sized.
