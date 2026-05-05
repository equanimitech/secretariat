# Project — v0.3 simplification (substrate + tray + MCP-primary)

**This is a simplification project, not a feature push.** Every slice
is evaluated by net delta — lines of code + surfaces added vs removed.
Net direction must be neutral or subtractive. See
`memory/project_v03_is_simplification.md` for the decision rule.

Replaces `docs/milestones/2026-05-05-menubar-and-quick-pane.md` and the
earlier "menubar with main window" framing.

Sources:
- `docs/pitches/2026-05-05-event-sourced-envelope-substrate.md`
- `docs/pitches/2026-05-05-menubar-only.md`
- Direction lock-in: `memory/project_mcp_is_primary_interface.md` —
  the Tauri app has NO main window, NO walker UI, NO settings panes,
  NO in-app composer. Claude (MCP) is the primary interface for all
  correspondence operations; the app is peripheral status + capture
  only.

The substrate pitch turns local captures (ideas, future pains, future
agent bids) into envelopes addressed to local queues. The menubar pitch
removes window chrome and adds a quick-pane for capture from anywhere.
They merge cleanly because the menubar's "ideas pool" (original slice
4) is exactly what the substrate's `Recipient::LocalQueue` provides — at
the cost of one enum, one newtype, one field, and one projection union
— while *deleting* a parallel data model that would have grown.

This is one merged project, not two parallel ones.

## Substrate update — queues are the primitive (2026-05-05, post-implementation)

Mid-slice-1 the substrate collapsed once more. The final shape is a
flat `Recipient { owner: Did, handle: QueueHandle }` struct — no enum
variants. Direct messages, local captures, and channel/newsletter
posts all share that shape; routing is by `owner == self_did?`
predicate, not by type. Stamps are allowed on any envelope. Wire
format: `to: <owner-did>` + `handle: <namespace:slug>`, both always
present. See `memory/project_substrate_simplifications.md` for the
rationale; the slice descriptions below have been updated to match.

## Simplification accounting

| Slice | Adds | Removes | Net |
|---|---|---|---|
| 1 — Substrate | `Recipient` struct, `QueueHandle`, `capture_to_queue`, projection union (~280 LOC, 9 tests) | `Option<Did>` recipient, `EnvelopeKind`, `Recipient` enum + variants, `allows_stamp` invariant, `as_peer_did`/`as_queue_handle`/`SelfAddressed`/`LocalQueue` (~80 LOC) + parallel data models (ideas pool, pains pool, agent-bid pool) | **subtractive in concept, modestly additive in code** |
| 2 — `/idea` skill migrates | `idea_capture` MCP tool (~40 LOC) | `Write to docs/ideas/*.md` skill body (~10 LOC); future per-skill capture handlers | **subtractive** |
| 3 — Tray icon | `TrayIconBuilder` setup (~80 LOC) | Original menubar pitch's popover plan (never built); two-buttons home in `<ReviewSurface>` (built, now removable) | **subtractive in plan, +~80 LOC code** |
| 4 — Window-less lifecycle + popover carve-out + 2-name profile | Lifecycle conditional (~30 LOC), NSPanel anchoring (~20 LOC), profile v2 schema migration (~30 LOC) | Persistent main-window assumption (already commented out, can now delete); auto-show on launch | **subtractive** |
| 5 — Quick-pane wired to capture | Replace template QuickPaneApp content (~100 LOC) | Template's placeholder QuickPaneApp body (~80 LOC); the previously-planned parallel ideas-pool storage (never built) | **near-zero net, repurposes existing scaffold** |
| 6 — MCP review tools | `defer_envelope` / `archive_envelope` MCP tools (~30 LOC) — wrap already-shipped application use cases | Walker UI plan (never built); in-app stamp button + envelope reader modal shipped earlier today (delete here) | **subtractive — deletes existing UI** |

**Project net:** small additive in raw LOC, **subtractive at every
other layer** — surfaces, parallel data models, cognitive load,
roadmap, planned-but-unbuilt UI. The codebase ends slightly larger
(~+160 LOC) but conceptually much smaller — one substrate, one
capture path, one review path. Subsequent versions stay simple by
discipline.

## The merged model — one primitive for everything

```
                Envelope.recipient = (owner: Did, handle: QueueHandle)
                            │
        ┌───────────────────┴───────────────────┐
        │                                       │
   owner == self_did                       owner != self_did
   (local capture)                  (peer DM, channel post, agent bid)
        │                                       │
   stays on disk                       deliver to owner's relay
        │                                       │
        ▼                                       ▼
   ~/.secretariat/queues/<ns>/<slug>/   ~/.secretariat/outbox/<sanitized_did>/
```

Stamps are allowed on **any** envelope. Tamper-evident self-attestation
of one's own captures is a valid use case. The send-routing rule
decides what *happens* to a stamped envelope.

Three real-world cases collapse to one shape:

- **Direct message** — `(peer_did, inbox:default)`. Default DM handle.
- **Local capture** — `(self_did, inbox:triage)` etc. Stays on disk.
- **Channel post** — `(publisher_did, channel:foo)`. Multi-subscriber
  polling on the relay side; domain-layer no-op.

Walker projection unions `outbox/<sanitized_did>/` AND `queues/<ns>/<slug>/`
— same markdown, same frontmatter, same domain entity, discriminated
at the UI by `to == self_did?`.

## Sequencing

Six slices. Strictly ordered: each depends on what's before. Stop at
any slice; partial release still useful.

### Slice 1 — Substrate (DONE 2026-05-05)

The wire-format change landed first because all UI choices depend on it.

**What changed (final, post-collapse):**
- `crates/core/src/domain/envelope.rs` — `Envelope.to: Option<Did>`
  becomes `Envelope.recipient: Recipient`. Wire format unified:
  `to: <owner-did>` + `handle: <namespace:slug>`, both always present.
  Legacy `to:`-only envelopes synthesize `inbox:default` on read.
- New value object `crates/core/src/domain/recipient.rs` — flat struct
  `{ owner: Did, handle: QueueHandle }`. No enum variants. `is_local(me)`
  predicate replaces variant matching.
- New value object `crates/core/src/domain/queue_handle.rs` — newtype
  parsing `^[a-z]+:[a-z0-9-]+$`, free-form namespaces.
- Stamp invariant **dropped**. Stamps allowed on any envelope including
  self-addressed local captures. `send_envelope` rejects only
  `owner == from` (would have nowhere to deliver to).
- Application layer: new `capture_to_queue` use case in
  `crates/core/src/application/capture_ops.rs`. Walker projection
  (`list_review_queue`) unions `outbox/<peer>/` AND `queues/<ns>/<slug>/`.
- Lexicon: `lexicons/tech.equanimi.secretariat.envelope` documents the
  unified `(to, handle)` pair.
- Surfaces wired: CLI `sec compose --to <did> [--handle <h>]`,
  `sec capture --queue <ns:slug>`. MCP `compose` (with optional
  `handle` param), MCP `capture`. Tauri `list_review_queue` unions
  both streams. tauri-specta bindings regenerated.
- Tests: 175 core (was 168 pre-substrate), v0_correspondence including
  a non-default-handle DM round-trip.

**Demo:** `sec capture --queue=inbox:triage --body="tell dad chapter 3
needs more pressure"` → file lands at
`~/.secretariat/queues/inbox/triage/<timestamp>.md`. The principal's
review session sees it alongside any unstamped peer drafts.

### Slice 2 — Migrate `/idea` skill to use the substrate (½ day)

Prove the substrate from the principal's most-used capture path before
building UI on top of it.

**What changes:**
- `~/.claude/skills/idea/SKILL.md` — rewrite the skill body. Instead of
  `Write` to `docs/ideas/<slug>.md`, call the MCP `capture` tool with
  `queue: inbox:triage` and the user's raw phrasing as body.
- Same treatment for `/pain` (queue: `inbox:pain`).
- Existing `docs/ideas/*.md` files stay on disk (no migration). New
  captures go to the substrate from this point forward.

**Demo:** in any Claude conversation, `/idea something I want to send
later` → idea lands in `~/.secretariat/queues/inbox/triage/`, visible in
the next review session. Old `docs/ideas/` files coexist as historical
record.

### Slice 3 — Tray icon (½ day)

Now we have a substrate; the app becomes peripheral.

**What changes:**
- `src-tauri/Cargo.toml` — add `tauri = { features = [...,
  "tray-icon", "image-png"] }`.
- `src-tauri/src/lib.rs` — `setup` hook installs `TrayIconBuilder`.
- Right-click menu items (no popover for v1):
  - "Capture an idea…" → opens the quick-pane (slice 5)
  - "Sync now" → calls `sync_now` directly
  - "How to onboard" (only when no identity yet) → copies prompt to
    clipboard
  - "Quit Secretariat"
- Left-click for v1: same as right-click (menu opens). Future option
  to show a small dropdown with counts; not v1.
- No popover window. No `<ReviewSurface>` UI lives in the app.

**Demo:** menubar icon visible. Right-click → menu of actions. None
of them open a window — they trigger MCP/CLI flows or the quick-pane.

### Slice 4 — Lifecycle: tray-popover for onboarding only + tray badge (½ day)

The principal's daily surface is the tray icon. The ONE exception:
first-launch onboarding shows the existing `<Onboarding>` component in
a tray-anchored popover (~400×500). It closes on completion and never
reappears — a bounded experience, not a persistent window.

**What changes:**
- `lib.rs` setup hook — never auto-show the main window. On first
  launch (no identity), open the onboarding popover anchored to the
  tray icon. On every subsequent launch, just install the tray and
  exit setup silently.
- Reuse the existing `<Onboarding>` component (already built — name +
  identity + optional invite). Wrap in a NSPanel-style popover via
  the template's `tauri-nspanel` plugin (same scaffolding the
  unused quick-pane uses).
- On `onComplete` callback → popover dismisses, tray icon transitions
  red → green (or amber if peer drafts already exist somehow).
- Background sync loop emits `tray:state-changed` event on each tick;
  Rust updates tray icon image (red / amber / green dot SVGs).
- Close-requested handler — quit on tray-only (Cmd+Q from tray menu).
- Right-click tray menu: "Capture an idea…" / "Sync now" / "Quit
  Secretariat". No "How to onboard" item — popover handles that
  natively for fresh installs.

**Demo:** install .dmg → drag to /Applications → open → tray icon
appears (red dot) + small popover slides down with two name fields:
*"Your full name"* (e.g. "Rafael Toletti Ballestiero") + *"How
would you like to be called?"* (e.g. "Rafa" — pre-filled from the
first word of the full name as you type, editable) → click "Set me
up" → identity generated → popover advances to "paste an invite URL
or skip" → popover closes on completion → tray dot transitions
red → green. Subsequent launches: just the tray icon, no popover.

**Profile data model change** (folds into slice 1's substrate work
since profile_store and Envelope share the domain crate): v1
profile (`{ version: 1, display_name }`) extends to v2
(`{ version: 2, full_name, display_name }`). v1 profiles loaded
get `full_name = display_name` as default migration. See
`memory/project_profile_two_names.md`.

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

### Slice 6 — MCP review tools (½ day)

Review happens in Claude, not in the app. The app's tray dot signals;
the principal asks Claude "review my inbox" / "review my outbox" /
"walk me through my queue"; Claude calls the right MCP tools and
guides the principal through the conversation.

**What changes:**
- New MCP tools wrapping the inbox-actions primitives shipped earlier
  today: `defer_envelope`, `archive_envelope`, `promote_idea_to_letter`.
- The `compose` MCP tool gains an optional `from_idea_id` parameter so
  Claude can promote an idea to a letter (read body, draft envelope to
  selected recipient, archive the original idea).
- No UI work in the Tauri app for this slice.

**Demo:** in Claude, "review my outbox queue" → Claude calls
list_review_queue + read for each draft → for each: shows body, asks
"stamp + send?" → on yes, calls stamp_envelope (Touch ID fires) → on
"defer", calls defer_envelope → walker ends when queue is exhausted →
principal sees tray dot transition green afterward.

## The full UX — a day in the life (MCP-primary)

The principal's day. Marcelo as audience, Rafa-as-author on the book.

**Morning** (anywhere — editor, browser, Slack):
- Thought hits: "tell Marcelo the constraint section needs a human
  example". `Cmd+Shift+S` → quick-pane → type → Enter. Idea lands in
  `inbox:triage`. **No window. No Claude prompt. 4 seconds.**

**Mid-day** (writing in Claude Code on the chapter):
- Claude is drafting prose. Rafa says "draft an envelope to Marcelo
  with this section's outline." Claude calls the `compose` MCP tool
  → unstamped letter lands in `outbox/<marcelo>/`. **No interruption.**

**EOD review** (Rafa's chosen review time, e.g. 6pm):
- Glances at menubar — tray dot is **amber** (has been since the
  morning capture).
- Opens Claude Code (already open from mid-day work) and types:
  *"Review my outbox queue."*
- Claude calls `list_review_queue` (sees 1 idea + 2 letter drafts).
  Walks one at a time, in Claude's chat:

  > Claude: "First up — an idea you captured this morning: '*tell
  > Marcelo the constraint section needs a human example*'. Promote
  > to a letter, archive, or skip?"
  > Rafa: "promote, to Marcelo."
  > Claude: [calls `compose` with `from_idea_id`] "Drafted. Body:
  > [shows the proposed letter]. Stamp + send?"
  > Rafa: "stamp."
  > Claude: [calls `stamp_envelope`] → Touch ID fires → "Stamped + sent.
  > Next."
  > [...repeat for the other two drafts]
  > Claude: "Queue clear."

- Tray dot transitions amber → green.

**Throughout, never:**
- App window opens. The app is the tray icon and the quick-pane,
  nothing else.
- Claude is replaced by an in-app composer. Drafting + reviewing live
  where Rafa already works (in Claude Code).
- Notifications fire. Tray dot is the only ambient signal.

**Onboarding** (a fresh install, e.g. for Christophe):
- Christophe opens `Secretariat.app` from /Applications → tray icon
  appears (red dot) + small popover slides down anchored under it.
- Popover shows "Welcome to Secretariat" + name field + Set me up
  button (the existing `<Onboarding>` component, repurposed).
- Christophe types "Christophe", clicks → identity generated locally
  → popover advances to "paste an invite URL or skip" → Christophe
  pastes the URL Rafa sent him → claim runs → popover closes.
- Tray dot transitions red → green.
- *No window. No copy-paste-into-Claude. Bounded experience that ends.*

**Settings** (rare):
- Rafa wants to update his name from "Rafa" to "Rafa B." → in Claude:
  "Change my Secretariat display name to 'Rafa B.'"
- Claude calls `set_profile` — done.
- Or via terminal: `sec profile set "Rafa B."`

## What changes in the v0.3 release scope

| Was (separate plans) | Is (merged + MCP-primary) |
|---|---|
| Menubar slice 1: tray + popover | **Slice 3** — tray only, no popover |
| Menubar slice 2: hide main window | **Slice 4** — no main window at all |
| Menubar slice 3: tray badge | Slice 4 (folded) |
| Menubar slice 4: ideas pool data model | **Replaced** by substrate slice 1 |
| Menubar slice 5: quick-pane | Slice 5 (still ships) |
| Substrate slice (whole pitch) | Slices 1+2 |
| Walker UI in app | **Cut** — review happens in Claude (slice 6 = MCP review tools instead) |

Net: 6 slices, same total work, sharper sequencing, **no main window
ever**, no parallel data models for "ideas" vs "envelopes", no walker
UI to maintain inside the app.

## Decision log

- **Substrate ships first.** Every other slice depends on Recipient +
  EnvelopeKind. Building anything else first would require rewrites.
- **`/idea` skill migrates BEFORE the quick-pane.** Proves the
  substrate from the most-trafficked capture path. Quick-pane in
  slice 5 then has a working backend.
- **No main window. Onboarding popover is the carve-out.** Daily
  use surface = tray icon + quick-pane. Onboarding is a one-shot
  ritual with a defined endpoint (equanimitech principle 6 —
  bounded experiences) so it gets a small tray-anchored popover
  reusing the existing `<Onboarding>` component. Closes on
  completion; never reappears unless reset. This is *not* a
  persistent window — it's an ephemeral panel that exists for one
  ritual.
- **No walker UI inside the app.** Review happens in Claude. Slice 6
  ships the MCP tools Claude calls; the conversation is the walker.
- **Tray dot is the only ambient signal.** Red = needs setup,
  amber = pending, green = clear. No notifications, no badges with
  numbers, no popovers.
- **`docs/ideas/*.md` files stay** as historical record. No migration
  script.

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
