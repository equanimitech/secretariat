---
name: review-repos
description: Altitude-aware review of stamped markdown directly on top of git repos — the git-native substrate's review walker, composed from `git` + `sec verify` + `sec stamp` with zero new infrastructure. Surfaces per-doc stamp state (sealed / unstamped / new / revised / tampered) across one or more repos, rendered coarse→fine so the principal descends on demand and seals only what they read in full. Use when the user says "/review-repos", "review the repos", "review docs in this repo", "what needs stamping", "what changed since I stamped it", or names a repo path ("review leggia", "review ~/Developer/..."). Companion to the git-native substrate (`docs/ideas/2026-05-31-git-native-substrate.md`). Distinct from `/review` (which triages captures in the legacy `~/.secretariat/` queues).
user-invocable: true
allowed-tools:
  [
    Bash,
    Read,
    Glob,
    Agent,
    mcp__secretariat__read,
    mcp__secretariat__stamp,
  ]
---

# Review over repos

The git-native review walker. The substrate is repos; this skill reviews the
markdown in them by **stamp state**, rendered as an **altitude ladder**
(coarse→fine). No daemon, no queue tree, no new crate — it composes three
primitives that already work:

```
git ls-files / status / log   → which docs · new · changed · over-time
sec verify --json <file>      → stamp state per doc (the integrity check)
sec stamp <file>              → seal at the floor (Touch ID)
```

`SEC` = the prod binary: `/Applications/Secretariat.app/Contents/MacOS/sec`
(never `./target/debug/sec` — see `memory/feedback_prod_binary_not_debug`).

## Scope resolution

| Signal | Scope |
| ------ | ----- |
| `/review-repos` (no args) | the current working directory's repo |
| `/review-repos <path>` | that repo |
| `/review-repos <name>` | match a known repo by name (e.g. `leggia` → `~/Developer/themia/leggia`) |
| multiple paths/names | cross-repo roll-up (altitude 1 = one line per repo) |

A future `~/.signet/repos.json` will hold the registered-repo set; until it
exists, take paths/names from the invocation (default: cwd).

## Which docs

Default scan = the repo's **doc surface**, not every `.md`:

- everything under `docs/` (and `decisions/`, `pitches/`, `ideas/`, `changelog/` if present)
- **plus** any `.md` anywhere that already carries a `$attestation:` frontmatter block (a stamped doc must always surface, wherever it lives)

Skip `node_modules/`, `.git/`, vendored trees, and tooling READMEs
(`.pytest_cache`, etc.). When unsure whether a dir counts as doc surface,
ask once rather than flooding the review with noise.

## Deriving state per doc

For each candidate file, run `sec verify --json <file>` and read git status.
Map to one of five states:

| State | Signal | Meaning |
| ----- | ------ | ------- |
| **SEALED** | `stamp.outcome = "verified"`, file unchanged since its commit | principal has vouched; body matches the sealed hash |
| **REVISED** | `stamp.outcome = "verified"` **but** `git status` shows the file modified (` M`) since commit | legitimately edited after sealing — a **re-stamp candidate** |
| **TAMPERED** | `stamp.outcome = "tampered"` (`claimedHash ≠ computedHash`) | body no longer matches the sealed hash. **Re-stamp if the change was intentional; investigate if not.** |
| **UNSTAMPED** | `stamp.outcome = "none"`, file is tracked | committed but never sealed |
| **NEW** | `stamp.outcome = "none"`, file is untracked / uncommitted (`??` in `git status --porcelain`) | freshly written, not yet in history |

Canonical commands:

```bash
SEC=/Applications/Secretariat.app/Contents/MacOS/sec
git -C "$REPO" ls-files '*.md'                         # tracked docs
git -C "$REPO" ls-files --others --exclude-standard '*.md'   # NEW (untracked)
git -C "$REPO" status --porcelain -- "$f"              # ' M' = changed, '??' = new
"$SEC" verify --json "$REPO/$f"                        # → {stamp:{outcome,...}}
git -C "$REPO" log -1 --format='%h %ci %s' -- "$f"     # over-time: last seal/commit
```

Note: verifying every doc is one `sec` call each — fine for a few hundred
docs. If a repo is huge, scope to `docs/` first and say so.

## The altitude ladder (rendering discipline)

This is `semantic-zoom` applied to the review surface. **Default to rendering
altitudes 0–1** (coarse). Descend one rung on the principal's signal; never
dump the floor unprompted.

| Altitude | Renders | Example |
| -------- | ------- | ------- |
| **0 — handle** | one glyph line, totals | `⚓ secretariat · 104 unstamped · 1 tampered · 2 new` |
| **1 — per repo / per state** | one line per repo (multi) or per state bucket (single) | `TAMPERED 1 · REVISED 0 · UNSTAMPED 104 · NEW 2 · SEALED 5` |
| **2 — per doc** | pick a bucket → one line per doc: path · state · last-commit · 1-line gist | `docs/ideas/git-native-substrate.md · NEW · (uncommitted) · "repos as channels"` |
| **3 — page** | pick a doc → headline + **`git diff` since last stamp** + claimed/computed hash | for the decide-to-seal moment |
| **4 — floor** | the full body verbatim → the stamp ceremony | only on explicit "stamp it" |

Lead with altitude 0–1. Always surface **TAMPERED first** (it's the integrity
alarm), then REVISED (re-stamp candidates), then NEW/UNSTAMPED, then SEALED
(usually collapsed to a count). Stop wherever the principal stops — the review
is anti-compulsion; not every doc must reach the floor.

## The floor — stamp ceremony (hard rule #4)

Sealing is the only state-changing act, and it is **principal-attested**.
Before calling `sec stamp` / `mcp__secretariat__stamp`:

1. Render the **full decrypted body verbatim** (altitude 4) — code block or quoted region, never a summary.
2. Get **explicit consent in the same turn** ("stamp it"). Prior-turn consent does not count if the file changed.
3. Then stamp. The Touch ID dialog carries the doc's first-line headline + a short hash prefix — if it differs from what you displayed, **abort**.

Re-stamping a REVISED/TAMPERED doc uses `sec stamp --force` (it already
carries an `$attestation`). Same ceremony.

This matches `memory/feedback_show_drafts_before_signing` and
`memory/feedback_confirm_handle_before_stamping`.

## Flow

1. **Resolve scope** → repo path(s).
2. **Walk** the doc surface; derive state per doc (commands above).
3. **Render altitude 0–1** — totals + state buckets, TAMPERED first.
4. **Wait** for the principal to descend ("show the unstamped", "what's tampered", "open the git-native note").
5. **Descend** one rung to what they pointed at — not the whole set.
6. **At the floor**, run the stamp ceremony only on explicit consent.
7. **Never** modify doc bodies, move files, or auto-stamp. Read + verify + (consented) stamp only.

## Rules

- **Read-only by default.** The only write is a consented `sec stamp`.
- **Never auto-stamp.** Show body verbatim + same-turn consent, every time.
- **TAMPERED is an alarm, not a chore** — surface it first, name the hash mismatch, ask whether the edit was intentional before offering re-stamp.
- **Expand `~`** before filesystem ops; always pass `-C "$REPO"` to git.
- **Prod binary only** for `sec` (`/Applications/Secretariat.app/Contents/MacOS/sec`).
- **Descend locally.** A follow-up about one doc descends that doc, not the whole repo.
- Don't invent a `repos.json` — if cross-repo scope is requested and no registry exists, take the paths from the user.
