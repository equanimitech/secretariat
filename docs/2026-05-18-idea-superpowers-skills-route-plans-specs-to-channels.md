---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T131039Z-4mrspq.md
type: idea
---

# Idea — /superpowers skills route plans/specs to channels, not repo docs/

**Raised:** 2026-05-18 mid-/review. Saving as idea for later shaping (per principal: "save routing as an idea for now").

## Pain (today)

/superpowers skills (`writing-plans`, `brainstorming`, `requesting-code-review`, `verification-before-completion`, `shaping`, etc.) write plans/specs/reviews to repo `docs/plans/`, `docs/specs/`. Consequences:
- Plans live in git, invisible to /review ceremony.
- No cross-repo plan visibility — `dev:leggia` and `dev:minerva` plans split between two repos' `docs/` trees.
- No stamps = no audit trail of which plan was endorsed.
- Drift between channel-curated narrative and repo-internal documents.

## Sketch (not shaped)

Route by skill class:
- **Channel (drafts in outbox)** — `writing-plans`, `brainstorming`, `requesting-code-review`, `receiving-code-review`, `verification-before-completion`, `shaping`. Land in `<repo>/.secretariat/outbox/<slug>.md` (or directly in `~/.secretariat/orgs/<org>/channels/dev:<repo>/outbox/`). Principal stamps to materialize.
- **Repo (code-coupled)** — `executing-plans`, `test-driven-development`, `subagent-driven-development`, test files, source files. Stays where it is.
- **Judgment** — `feature-dev:code-architect` blueprints. Channel if shape-up-like, repo if evergreen spec.

## Mechanism

- **Binding resolver** — skill checks `cwd → repo-root → (org, channel)`. Three candidate sources:
  1. Per-org `repos.toml` declaring `repos = { leggia = "themia.pro/dev:leggia" }` (clean, explicit).
  2. Symlink at `<repo-root>/.secretariat → ~/.secretariat/orgs/<org>/channels/dev:<repo>/` (filesystem-level, repo-init does this).
  3. Git-remote URL → org DID → channel inference (auto, fragile).

  Rafa said "we already have the binding paths" — needs follow-up on which mechanism he meant.

- **Skill wrapper** — patch /superpowers skill outputs to check for binding → write to outbox → fall back to `docs/` if no binding. Could be a single hook applied to the skill family.

- **Channel-side CLAUDE.md** — the `module:baux_commerciaux/.claude/CLAUDE.md` pattern (saw it injected during today's review) governs agent behavior inside the channel dir. Extend to declare allowed streams + skill defaults: `streams_accepted: [plan, spec, review, pitch]`.

## Why not now

- Need to confirm which binding mechanism already exists.
- Each /superpowers skill has its own contract; patching all of them needs care.
- Stream vs tag schema resolution (see infra ask envelope earlier today) probably blocks clean stream-tagging.

## Composes with

- `dev:` parent reorg (decided today).
- Tags as first-class axis (today's infra ask).
- Channel-CLAUDE.md role specs (the BC Vérificateur Veriguard pattern).
