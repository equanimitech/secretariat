# Project — Substrate + menubar (v0.3 merged plan)

Replaces `docs/milestones/2026-05-05-menubar-and-quick-pane.md`.

Sources:
- `docs/pitches/2026-05-05-event-sourced-envelope-substrate.md`
- `docs/pitches/2026-05-05-menubar-only.md`

The substrate pitch turns local captures (ideas, future pains, future
agent bids) into envelopes addressed to local queues. The menubar pitch
removes window chrome and adds a quick-pane for capture from anywhere.
They merge cleanly because the menubar's "ideas pool" (original slice
4) is exactly what the substrate's `Recipient::LocalQueue` provides — at
the cost of one enum, one newtype, one field, and one projection union.

This is one merged project, not two parallel ones.

## The merged model — H↔H + H↔A on the same primitive

```
                        ┌────────────────────┐
                        │    Envelope.to     │
                        └─────────┬──────────┘
                                  │
                  ┌───────────────┴───────────────┐
                  ▼                               ▼
         Recipient::Peer(Did)         Recipient::LocalQueue(handle)
              │                                   │
       (crosses H↔H boundary)            (stays inside the principal)
              │                                   │
   stamp eventually required             stamp forbidden by invariant
              │                                   │
              ▼                                   ▼
   ~/.secretariat/outbox/<did>/.md      ~/.secretariat/queues/<handle>/*.md
```

`EnvelopeKind = Letter | Idea` (v1). Walker projection unions both file
trees — same markdown, same frontmatter, same domain entity, different
recipient kind.

## Sequencing

Six slices. Strictly ordered: each depends on what's before. Stop at
any slice; partial release still useful.

### Slice 1 — Substrate (1 day)

The wire-format change lands first because all UI choices depend on it.

**What changes:**
- `crates/core/src/domain/envelope.rs` — `Envelope.to: Option<Did>`
  becomes `Envelope.recipient: Recipient`. New enum
  `Recipient::Peer(Did) | Recipient::LocalQueue(QueueHandle)`. Existing
  serialization for `Peer` matches today's `Some(Did)` byte-for-byte.
- New value object `crates/core/src/domain/queue_handle.rs` — newtype
  parsing `^[a-z]+:[a-z0-9-]+$`, only `inbox:*` recognized in v1.
- New field `Envelope.kind: EnvelopeKind` (`Letter` default for back-compat,
  `Idea` for local-queue captures).
- Domain invariant: `Recipient::LocalQueue` rejects stamps at construction.
- Application layer: new `capture_to_queue(handle, kind, body)` use case
  in `crates/core/src/application/capture_ops.rs`. Walker projection
  (`list_review_queue` / `list_outbox_queue`) extended to union
  `outbox/<peer>/` AND `queues/<handle>/`.
- Lexicon: update `lexicons/tech.equanimi.secretariat.envelope` with the
  new fields. Schema is mutable until self-use validates.
- Tests: domain (4-6 cases for Recipient/QueueHandle/invariants),
  application (capture roundtrip, projection union), MCP boundary.

**Demo:** `sec capture --kind=idea --queue=inbox:triage "tell dad chapter
3 needs more pressure"` → file lands at
`~/.secretariat/queues/inbox/triage/<timestamp>.md`. `sec list review`
(or the existing list_review_queue) shows the idea alongside any
unstamped peer drafts. Doesn't touch the app yet.

### Slice 2 — Migrate `/idea` skill to use the substrate (½ day)

Prove the substrate from the principal's most-used capture path before
building UI on top of it.

**What changes:**
- `~/.claude/skills/idea/SKILL.md` — rewrite the skill body. Instead of
  `Write` to `docs/ideas/<slug>.md`, call `sec capture --kind=idea
  --queue=inbox:triage` with the user's raw phrasing as body.
- Add MCP tool `idea_capture` (or just `capture`) wrapping the
  application use case so the skill can call it via tauri-specta when
  Secretariat is wired into Claude.
- Existing `docs/ideas/*.md` files stay on disk (no migration). New
  ideas go to the substrate from this point forward.

**Demo:** in any Claude conversation, `/idea something I want to send
later` → idea lands in `~/.secretariat/queues/inbox/triage/`, visible in
the next review session. Old `docs/ideas/` files coexist as historical
record.

### Slice 3 — Tray icon + minimal popover (½ day)

Now we have a substrate; the app starts becoming peripheral.

**What changes:**
- `src-tauri/Cargo.toml` — add `tauri = { features = [...,
  "tray-icon", "image-png"] }`.
- `src-tauri/src/lib.rs` — `setup` hook installs `TrayIconBuilder`.
  Click toggles main window visibility (lifecycle hardening in slice 4).
- New tray-popover window definition in `tauri.conf.json`,
  capabilities updated (`tray-popover` window).
- `src/components/secretariat/TrayPopover.tsx` — wraps the existing
  `<ReviewSurface>` two-button home in a small popover layout.

**Demo:** menubar icon visible. Click → popover slides down with the
familiar two buttons. Main window also still opens normally on launch.

### Slice 4 — Lifecycle: hide main window post-onboarding + tray badge (½ day)

The window goes away. Tray dot shows queue state.

**What changes:**
- `lib.rs` setup hook — check `current_identity` + `get_profile`; hide
  main window if both present (post-onboarding state).
- Background sync loop emits `tray:state-changed` event on each tick;
  Rust side updates tray icon's image (green dot template / amber dot
  template). Two static SVGs in `icons/`.
- Close-requested handler — quit on tray-icon-only mode (Cmd+Q from
  tray menu) instead of just hiding.

**Demo:** launch fresh → wizard appears (main window) → finish onboarding
→ window vanishes → only tray icon. Tray dot is amber when there's
anything in inbox or queue (peer drafts OR local-queue captures);
green when both are empty.

### Slice 5 — Quick-pane wired to `sec capture` (1 day)

The capture entry point — the half of the menubar pitch that survives
the merge.

**What changes:**
- `src/components/quick-pane/QuickPaneApp.tsx` — replace template
  content with single text field + (optional) recipient picker (peer
  contact OR local queue). "Capture" button.
- Default action: when no recipient picked, lands in `inbox:triage` as
  `kind=idea`. With a peer DID picked, creates an unstamped letter
  draft in `outbox/<did>/`.
- Tauri command `capture_from_pane(body, recipient_choice, kind)` —
  thin wrapper that calls either `compose_envelope` or
  `capture_to_queue` based on recipient kind.
- Default global shortcut: `Cmd+Shift+S` (configurable, replaces
  template's `Cmd+Shift+.`).
- Tray-popover gains a "Capture…" button as alternative entry point.

**Demo:** any window focused → `Cmd+Shift+S` → small pane appears →
type "tell dad chapter 3 needs more pressure", leave recipient blank →
Enter → pane dismisses → idea is in the principal's `inbox:triage`
queue, visible at next review.

### Slice 6 — Walker handles both kinds (½ day)

The review session natively understands letters and ideas.

**What changes:**
- `<ReviewSession>` walker (still to-be-built per the pitch
  `docs/pitches/2026-05-05-inbox-review-walker.md` for inbox; sibling
  for outbox+queue) gets per-kind action bars.
- For `Letter` items in the outbox queue: actions = Stamp+Send / Defer
  / Archive / Next.
- For `Idea` items in `inbox:triage`: actions = Promote to letter /
  Archive / Next. "Promote to letter" opens the recipient picker;
  promotion creates an `outbox/<did>/<timestamp>.md` envelope with
  body copied, marked as `kind=Letter` requiring stamp; the original
  idea is archived.

**Demo:** open the app at chosen review time → "Drafts & ideas"
walker shows whatever's pending (mixed peer drafts + ideas) → for
each, the appropriate action bar → walk through, walker ends at
queue empty → tray dot transitions amber → green.

## The full UX — a day in the life

This is the principal's day with the merged design. Marcelo as the
audience, Rafa-as-author working on the book.

**Morning** (anywhere — editor, browser, Slack):
- A thought hits: "tell Marcelo the constraint section needs a
  human example". `Cmd+Shift+S` → quick-pane → type → Enter. Idea
  is in `inbox:triage`. No window opened, no Claude prompt, no
  context switch. Total: 4 seconds.

**Mid-day** (working in Claude Code on the chapter):
- Claude is drafting prose. At one point Rafa says "draft an envelope
  to Marcelo with this section's outline." Claude calls the
  `compose` MCP tool → unstamped letter draft lands in
  `outbox/<marcelo>/`. No interruption to Rafa's writing flow.

**EOD review session** (chosen time, e.g. 6pm):
- Tray dot is amber — has been since this morning's capture.
- Click tray icon → popover slides down with two buttons. Counts
  show "3 to review" total (1 idea, 2 letter drafts).
- Click "Drafts & ideas" → walker opens.
- Envelope 1 (idea, "tell Marcelo constraint section needs human
  example") → Promote to letter → recipient picker shows Marcelo →
  Promote → letter draft now in outbox queue with body copied.
- Envelope 2 (letter to Marcelo, this morning's outline) → Stamp+Send
  → Touch ID → relay queues. Walker advances.
- Envelope 3 (the just-promoted letter from idea 1) → Stamp+Send →
  Touch ID → sent.
- Walker ends; tray dot transitions to green.

**Throughout, never:**
- Window is the focus. App is in the periphery (tray icon, dot color).
- Notifications fire. The principal *visits* the surface; it doesn't
  visit them.
- The principal types into a textarea inside the app for a multi-
  paragraph composition. The AI assistant drafts; the principal
  triages.

## What changes in the v0.3 release scope

| Was (separate plans) | Is (merged) |
|---|---|
| Menubar slice 1: tray + popover | Slice 3 |
| Menubar slice 2: hide main window | Slice 4 |
| Menubar slice 3: tray badge | Slice 4 (folded in) |
| Menubar slice 4: ideas pool data model | **Replaced** by substrate slice 1 |
| Menubar slice 5: quick-pane | Slice 5 |
| Substrate slice (whole pitch) | Slices 1+2 |

Net: 6 slices instead of 5+1=6. Same total work, sharper sequencing,
no parallel data models for "ideas" vs "envelopes".

## Decision log

- **Substrate ships first.** Every UI choice (walker action bars,
  quick-pane recipient picker) depends on the Recipient + EnvelopeKind
  primitives. Building UI before substrate would require a rewrite.
- **`/idea` skill migrates BEFORE the quick-pane.** Proves the
  substrate from the most-trafficked capture path while UI is still
  template-shaped. Quick-pane in slice 5 then has a working backend
  to call.
- **Tray icon ships AFTER the substrate but BEFORE the walker
  rebuild.** Order: substrate → /idea wired → tray + lifecycle →
  quick-pane → walker per-kind. The walker change is small (one new
  action bar variant) so it lands last — right after the rest of the
  surface is shaped.
- **`docs/ideas/*.md` files stay** as historical record. No
  migration script. New ideas use the substrate; existing files are
  archive.

## Out of merged scope (future pitches)

- Migrating `/pain`, `/roundtable`, `/share` to the substrate.
- A↔A traffic — agents writing to queues with proper authorization.
- Cross-principal queue addressing.
- Append-only event log infrastructure.
- Daily-routine signals (`docs/ideas/daily-review-routines.md`) —
  blue-dot at configured times. Composes naturally on top of the tray
  badge from slice 4.
- Quote-reply / reply-to-parts.
- Multi-granularity envelopes.
- Channels as broadcast feeds.
- Native AI panel inside the app.

All those captured ideas remain valid; they get easier to ship once the
substrate exists.
