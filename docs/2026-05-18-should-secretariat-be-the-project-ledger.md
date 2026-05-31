---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T093041Z-h23sz3.md
---
# Should Secretariat be the project ledger?

**Status:** open design question, parked
**Surfaced:** 2026-05-18 alongside channel-map slice-A shaping (in [[secretariat:editor:ideas]])

## The question

"Later projects" — shaped designs awaiting execution, parked initiatives, things to come back to — are currently homeless in the substrate. They live as filesystem docs (`docs/superpowers/specs/`, `docs/superpowers/plans/`, `docs/ideas/`, `docs/pitches/`) with no in-Secretariat trace. Captures get me a parking-cursor in the queue but no project STATE (status, transitions, ownership).

Should Secretariat carry that state itself? Two design poles:

## Pole A — Thin pointer model (status quo, extended)

Secretariat captures stay thin: a capture envelope is a pointer to the design doc. Status / state / transitions live in the filesystem doc (e.g. `status: draft | parked | ready | in-progress | done` in frontmatter). `/review` surfaces captures; you triage. Promotion = edit the doc's status field + maybe stamp a "starting work" envelope.

**Pros:** keeps Secretariat boring. No new primitive. Doc filesystem already authoritative.
**Cons:** no native sense of "what's in flight," no cross-project query, status drift invisible.

## Pole B — Stamped state envelopes

A "project" becomes a sequence of stamped envelopes on a project channel (`equanimi.tech#project:channel-map-nav`). Each transition is a new stamped envelope: `shaped`, `parked`, `started`, `paused`, `done`, `dropped`. Latest stamped envelope = current state. Selective-stamp model fits: project state changes ARE commitments worth signing.

**Pros:** ledger native; cross-project query is just channel scan; transitions are cryptographically attested decisions; matches autonomous-enterprise framing (the org's decisions live on its channels).
**Cons:** ceremony cost per transition (Touch ID for "I started this"); risk of forcing every project into the substrate when filesystem-pointer would do; could degenerate into todo-app-with-extra-steps.

## Hybrid sketch

Pointer envelope at capture time (Pole A). Stamped state-transition envelope only at meaningful inflection points (Pole B for `started`, `paused`, `dropped`, `done` — not for fine-grained status). Filesystem doc remains authority for content.

## Why this matters

Recursive validation: the book *Autonomous Enterprise* is partly *about* how the org's decisions become first-class artifacts. If Secretariat is the substrate of that enterprise, project-level state IS what the enterprise commits to. Filesystem-only state breaks the loop.

## Not blocking

Channel-map slice A can ship without resolving this. Worth a real design conversation when the appetite arrives — possibly via the `research-assistant` or `/shaping` pattern.

## Adjacent

Cf. zenborg's cycle/cyclePlan primitive (Areas → Cycles → Plans → Moments) — partial overlap with "project state over time." Worth checking whether zenborg already has the right primitive and Secretariat just needs an adapter.
