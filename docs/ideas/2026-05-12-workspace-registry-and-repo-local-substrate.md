# Workspace registry — `.secretariat/` as a repo-local marker

**Date:** 2026-05-12
**Tags:** `equanimitech/secretariat`, v0.3+, substrate
**Status:** idea — captured during slice 1 shaping, deferred from slice 1

---

## The pitch in one paragraph

Any directory with a `.secretariat/` subdir IS a Secretariat workspace — a
project-local extension of the passport-home substrate. The publishable
state of a channel (skills, agents, CLAUDE.md, channel-def, roster
references) lives inside the repo and rides along with `git clone`. The
private state (keys, profile, drafts, decrypted history) stays in the
passport home and never enters the repo. Workspaces register with the
passport (`sec workspace register .`) and discover via upward-walk like
git, then merge into the unified channel tree at runtime.

---

## Why this matters

- **Project context follows code.** A `channel/secretariat/dev/` whose
  skills + CLAUDE.md live in the secretariat repo means every clone of
  the repo has the channel's full activation surface. No manual
  re-creation per machine.
- **Multi-machine principal.** Work laptop + personal laptop share
  project channels by git. Decrypted history doesn't ride the repo
  (private), but the daemon re-syncs envelopes from the channel owner's
  relay on first run.
- **Multi-principal onboarding.** Marcelo clones the _Autonomous
  Enterprise_ book repo → registers it → the `channel/autonomous-
enterprise/draft/` channel-dir is fully populated with committed
  skills (verify-citation, check-pacing, etc.). The book substrate IS
  the channel substrate. Recursive validation tightens.
- **Project-vs-personal separation is real.** Some channels are
  personal-only (journal, BYOK notes) — those stay in the passport
  home's `channel/`. Project-anchored channels live alongside the code.

---

## What lives in the repo vs the passport home

```
~/code/secretariat/
├── .git/
├── crates/
└── .secretariat/                       ← workspace marker
    ├── workspace.json                  # {name, owner_did, registered_channels}
    ├── channel/
    │   └── secretariat/dev/
    │       ├── CLAUDE.md               ✓ committed (channel context)
    │       ├── .claude/                ✓ committed (skills/agents/commands)
    │       ├── _meta/                  ✓ committed (channel def + roster as envelopes)
    │       ├── envelopes/              ✗ .gitignore (decrypted history is private)
    │       ├── outbox/                 ✗ .gitignore (drafts private)
    │       └── _ciphertext/            ✗ .gitignore (encrypted blobs — same as plaintext leak)
    └── .gitignore
```

| In repo                                           | In passport home                                  |
| ------------------------------------------------- | ------------------------------------------------- |
| `CLAUDE.md`, `.claude/skills/`, `.claude/agents/` | `key`, `did`, `profile.json`                      |
| `_meta/` (signed roster + channel def envelopes)  | `contacts.json`                                   |
| `workspace.json` (workspace identity)             | `attention-envelope.md`                           |
| Channel definitions, template files               | `outbox/`, `_ciphertext/`, decrypted `envelopes/` |

---

## Registration + discovery

Two mechanisms, both supported:

**Explicit registration** in the passport home:

```
~/.secretariat/rafa/workspaces.json
[
  { "id": "secretariat-dev",
    "path": "~/code/secretariat",
    "registered_at": "2026-05-..." }
]
```

CLI:

- `sec workspace register .`
- `sec workspace list`
- `sec workspace unregister <id>`

**Implicit upward-walk discovery** (like `.git/`):

When `sec` or MCP runs from `cwd`, walk up looking for `.secretariat/`.
If found AND registered, the workspace's channels become visible
alongside the passport-home channels for that session.
`cd ~/code/secretariat && sec capture --queue channel:secretariat:dev`
lands the envelope inside the **repo's** workspace, not the home tree.

Conflict rule: if a channel exists in both home tree and a registered
workspace, the workspace wins (project-local override, same precedence
model as `*.local.md`).

---

## What this changes upstream

- `KeyPaths` (slice 1 will probably rename to `Substrate` or
  `SubstrateRoot`) becomes a _list of substrate roots_ — passport home
  - N registered workspaces. Channel resolution iterates.
- `sec review` walks the merged tree.
- MCP `secretariat://review` resource emits the merged tree.

---

## Open questions

1. Encrypted `envelopes/` committable as opt-in (e.g. for public audit
   trails on a public repo) — leave it as workspace.json toggle?
2. Workspace ownership semantics — can a workspace serve channels for
   multiple orgs (e.g. a meta-repo for cross-org coordination)? Or is
   one workspace pinned to one owner DID?
3. Schema for `workspace.json` — should it itself be a signed envelope
   (substrate-uniform) or a plain JSON config?
4. Discovery permissions — if I `cd` into someone else's cloned repo
   with their `.secretariat/`, do I see their channels passively, or
   does it require explicit `sec workspace register` per principal?

---

## No-gos

- No central workspace registry. Each principal manages their own list.
- No automatic git sync of `_meta/` updates — `git pull` is the user's
  responsibility. Substrate doesn't push to git on its own.
- No multi-tenant workspaces (two principals sharing a `.secretariat/`
  with both their keys inside). One workspace = one publishable
  identity at most; private keys never enter the repo.
