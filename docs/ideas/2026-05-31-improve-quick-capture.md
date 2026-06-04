# Improve quick capture (Things3-grade)

**Date:** 2026-05-31 · re-grounded 2026-06-04
**Status:** someday
**Source:** Things3 quick-capture, triaged 2026-05-31; re-grounded on the repo
registry + Conductor onboarding teardown ([[2026-06-04-conductor-teardown]]).

> we need to improve quick capture for secretariat following things3:

Capture should be as frictionless as Things3 quick-entry. The original checklist
was **channel-era** (pre-v0.12.0 teardown) — it routed to
`~/.secretariat/orgs/.../channels`. The substrate is now **repos** (the
`[[repos]]` registry shipped 2026-06-01). Re-grounded:

- [ ] launch (interactive) in any registered repo
- [ ] dispatch (background, headless) into any registered repo
- [ ] capture a one-liner that *becomes* a launch/dispatch target

Meta-note: this whole inbox-triage exercise is the use case — fast capture is
worthless without fast routing.

## The model (2026-06-04)

The repo registry is the missing piece — it's the list of repos to pick from.
Quick capture is the universal entry point over it:

```
Global pane (anywhere) → type intent → pick repo (from [[repos]]) → ┬ Launch  (drive now, foreground)
                                                                     └ Dispatch (fire+forget, background)
```

Two verbs, one mouth:

| | **Launch** | **Dispatch** |
|--|--|--|
| Mode | interactive, foreground | headless, background |
| Principal | drives Claude in repo `cwd` | fires task, walks away |
| State today | `sec launch` exists but **channel-era** — needs repo-registry rewire | **never built** — the headless half |
| Result | live session | draft / branch reviewed later (feeds the review-session model) |

## The Conductor onboarding lesson — zero-config scribe

Conductor auto-recognizes Claude on first run (reads `~/.claude`, no setup) and
just works. Apply it: `[cognition]` is config-driven (invariant #4 — pluggable),
but **pluggable ≠ must-configure**. On `sec init`, *detect `~/.claude` + the
`claude` CLI, default the scribe to claude-code, don't ask.* Override stays.
Sovereignty over cognition doesn't require friction at the door.

## Slice ordering (keel-scoped)

1. **Zero-config scribe** — detect Claude on init, default it. Small, high-leverage.
2. **Re-ground `sec launch` on the repo registry** — drop channel resolution,
   resolve `path` from `[[repos]]`. (Launch doc `docs/developer/launch.md` is
   still channel-framed — update in the same slice.)
3. **Build `dispatch`** — the headless counterpart. Bigger; its own pitch.
4. **Quick-pane → repo picker → launch/dispatch** — wire the mouth. Repurpose the
   template's `quick_pane.rs` NSPanel (see [[quick-pane-for-message-ideas]]).

**Keystone slice = #1 + #2** (small, unblocks the rest). #3 / #4 simmer.

Related: [[document-as-workflow-node]] (doc-as-input to a skill is the *in-editor*
sibling of dispatch-into-repo), [[scribe-background-journaling]] (scribe-initiated
capture is the auto sibling of this principal-initiated pane).
