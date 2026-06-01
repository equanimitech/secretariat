---
migrated_from: equanimi.tech/project/secretariat/dev/20260517T200130Z-sv5rkk.md
---
# Substrate fix: standardize channel metadata file to `channel.md` (drop `.channelDef`)

## Problem observed (2026-05-17)

The substrate currently has two channel-metadata file formats coexisting:
- **`channel.md`** — used by older / legacy channels (e.g. `channel:jurimetria`, `channel:secretariat:dev`, `channel:journals:reviews`). Created by some earlier path.
- **`.channelDef`** — written by the current `create_channel` MCP tool (e.g. `channel:jurimetria-lab`, `channel:articles`).

The split causes desync bugs:
- `capture` to a `channel.md`-only channel fails with `channel does not exist` after the daemon rebuilds its in-memory index (e.g. after another `create_channel` call). Re-running `create_channel` on the same handle re-writes `.channelDef` → captures resume.
- Hit during 2026-05-17 review session on at least three channels: `channel:jurimetria`, `channel:secretariat:dev`, `channel:journals:reviews`. Same fix worked each time.

## Directive (per Rafa, 2026-05-17)

**Use `channel.md` for everything.** Drop `.channelDef`. Rationale: `channel.md` is human-readable, edit-friendly, consistent with the markdown-everywhere principle. `.channelDef` is a parallel hidden-file format that adds nothing.

## Work needed

- Migrate `create_channel` MCP tool + `sec channels create` CLI to write `channel.md` instead of `.channelDef`.
- Update daemon channel-discovery to read `channel.md` exclusively (or accept both during migration window).
- One-shot migration script: walk `~/.secretariat/orgs/*/channels/**/.channelDef` and `~/.secretariat/channels/**/.channelDef`, convert to `channel.md`, delete the hidden file.
- Update docs (`AGENTS.md`, channel-format reference) + skill descriptions.
- Tests: channel created with new tool round-trips through `list_channels` + `capture` + `read_channel`.

## Format proposal for `channel.md`

```markdown
---
$type: tech.equanimi.secretariat.channel
handle: channel:articles
org: equanimi.tech
name: Articles
description: Equanimitech write-ups — Torchbearer principles, AI-native UI thesis, public essays.
created_at: 2026-05-17T19:56:10Z
---

# Articles

(optional free-form channel notes / contract preamble below the frontmatter)
```

Frontmatter mirrors envelope schema for substrate consistency; body is operator notes.

## Why this matters now
- Live ergonomic friction during review sessions (this one).
- Pre-channel-era channels (created via legacy CLI/code path) are first-class citizens that should keep working without re-running `create_channel` as a side-effect.
- Aligns with the principle that the substrate IS the markdown layout — no hidden state files.

— captured during 2026-05-17 review session as a directive after hitting the bug 3× in one hour.
