# Layman UI — folders with AI in them

Pitch — 2026-05-19. Source: `/Users/rafa/.secretariat/_self/channels/inbox/ideas/envelopes/2026/05/19/20260519T180720Z-xpy5cw.md`

## Boundaries

### Job to be done
As a layman principal (someone who has never heard of envelopes, DIDs, or queues) sitting down at their Mac with Secretariat installed, I want to see *a list of folders, each with an AI assistant attached*, so that opening a session means picking a folder and starting to talk — no protocol vocabulary on the surface.

Baseline today: the Tauri shell at `src/components/layout/MainWindow.tsx` already navigates orgs (`OrgPicker.tsx`) and surfaces a `ReviewSurface.tsx`; the substrate is already filesystem-authoritative (per `project_filesystem_authoritative` and `project_channel_dir_is_activation_surface`) — channel-dirs are literally Claude Code projects. But the UI still speaks substrate dialect: "channels," "orgs," "envelopes," "stamp." A non-developer sees four nouns where they expect one.

### Appetite
`medium`

Appetite picked: `medium` — this is a vocabulary + activation-gesture pass over the existing Tauri shell, not a re-architecture. Three surfaces touch (sidebar list, activation gesture, stamp label) but no substrate changes. Override with `--appetite=<size>`.

## Elements

- **Place:** *Folder list* — the existing `LeftSideBar.tsx` re-rendered with channel-dirs as folders, nested by handle tree (`dev:relay` shown as `Dev / Relay`). Word "channel" disappears from labels; absolute filesystem path shown on hover as a teaching affordance ("this folder lives at `~/.secretariat/orgs/themia/channels/dev/relay/`").
- **Affordance:** *Open this folder's assistant* — single primary button per folder row. Wraps the existing `sec launch` (`docs/developer/launch.md`) call: `cd <channel-dir> && claude`. No "compose," no "channel actions" — one gesture per folder.
- **Affordance:** *Approve* — what the principal sees when an envelope needs stamping. Same Touch-ID ceremony underneath (AGENTS.md rule #4 unchanged: show body verbatim, confirm in same turn, Touch ID gate). The word "stamp" stays in CLI / MCP / lexicon; UI says "Approve." The cryptographic act is identical.
- **Connection:** *Drag a file into a folder* → it becomes a file in the channel-dir, picked up by the daemon's `OutboxWatcher` like any other artifact. No "attach to envelope" affordance — the folder IS the surface.
- **Place:** *Mailbox row* (top of folder list) — a single "Inbox" folder rolling up undecided items across all channel-dirs. Surfaces unread + pending-approval count. Replaces the orgs-then-channels two-step navigation for first-run.

## Risks

### 🐇 Rabbit holes
- *Native Finder integration as the primary surface.* Tempting — make every channel-dir literally appear in Finder sidebar via macOS bookmarks, double-click activates Claude. But: (a) Finder doesn't know about the daemon, (b) layman doesn't know about Finder either, (c) "open the Secretariat app, see your folders" is one click farther but one concept fewer. Defer Finder integration to a later pitch.
- *Hiding the channel handle entirely.* If a folder is named "Dev / Relay" but the underlying handle is `dev:relay`, what happens when the user renames the folder in Finder? Channel-dirs are still substrate; handles are stable IDs. Decision: rename gesture lives in MCP (`set_channel_label` style), Finder rename is ignored (or reverted by daemon). Cite this explicitly to engineering.
- *Multi-org navigation.* The mental model "list of folders" breaks when there are six orgs. Slack solves this with a workspace switcher. We already have `OrgPicker.tsx`. Question: does layman version need orgs at all, or is "folders I belong to" enough? Lean toward *single flat folder list, org grouping as visual section headers*, not a separate picker step.

### 🏴 Off-sides called
- Out: re-skinning MCP tool names. `compose` / `capture` / `stamp` stay as MCP verbs — layman never sees them directly. The UI surfaces the gestures, MCP carries the verbs. Per `project_mcp_is_primary_interface`: UI navigates, MCP handles CRUD.
- Out: changing lexicon `$type` strings, envelope shape, contract semantics. Substrate is invariant.
- Out: building a "create new folder" wizard. Folders come from `create_channel` (existing MCP tool) and from accepting invites. The UI lists what exists; it doesn't onboard new channels (that's a separate concierge surface).
- Out: cross-channel queries / multi-org dashboards. Those want a different surface (search, agent-driven digest); the folder UI deliberately does not surface them. If the layman needs them, they're not the layman anymore.

### 🥩 Fat cut
- *Per-folder color / emoji customization.* Already pitched separately (`2026-05-17-channel-emojis.md`). Don't fold in.
- *Folder preview on hover* (showing recent envelopes). Tempting visual affordance; cuts the simplicity. The whole point is *open the folder to see what's there*. Hover-preview reintroduces the substrate peek the layman shouldn't need.
- *In-app review surface for envelope-by-envelope triage.* `ReviewSurface.tsx` exists for the principal-as-operator; in the layman framing, review IS opening the folder's assistant. Don't duplicate the review affordance into the folder list.

### 🧪 Domain knowledge
- *Does "Approve" carry the right gravitas?* The stamp is a biometric, legally-meaningful attestation. "Approve" reads softer than "Stamp" — that's the layman point, but cross-check with Marcelo and Christophe before locking the label. Candidate alternatives: "Authorize," "Sign," "Confirm." (Per `feedback_no_book_examples` — don't invent book examples to justify; just test the word.)
- *Is the activation gesture `sec launch` enough?* It opens Claude Code in the directory. The layman expects something like "open a chat window." Today `sec launch` opens a terminal-hosted Claude. The layman version needs Claude Code's GUI host, OR a Tauri-hosted chat panel that talks to the same MCP. Domain check: does Claude Code expose a non-terminal entry point for "open this directory as a project" that the Tauri shell can invoke? Verify before betting the activation gesture on it.
- *What about the substrate that doesn't fit the folder metaphor?* Roster mutations, contract changes, invite acceptance — these are channel-scoped but not file-shaped. They surface naturally inside the folder's AI session ("add Christophe to this folder" → AI calls `invite` MCP tool). The folder UI itself doesn't need a roster pane.

## Pitch

### Problem
Secretariat's substrate is already shaped right for the layman — channel-dirs are folders, agents activate on `cd`, the filesystem is authoritative. But the *surface* still speaks engineer-dialect. The Tauri shell shows "Channels," "Envelopes pending stamp," "Org picker." A non-developer principal — Marcelo, Christophe, dad — sees four protocol nouns where they expect one folder list.

The collapse the idea proposes is real: from the layman's seat, every meaningful Secretariat verb maps to *"open this folder, talk to its assistant."* Compose → talk in the folder. Capture → drop a file in the folder. Stamp → approve when asked. Roster → invite someone to the folder. The substrate stays exactly as it is; the vocabulary contracts.

The risk of *not* doing this: every layman onboarding repeats the v0.4 Marcelo experience (`feedback_marcelo_first_attempt`) — clunky, opaque, four nouns the user has to learn before sending one message. The substrate has already grown past that; the surface hasn't caught up.

### The bet
A medium-sized pass on the Tauri shell to render the layman surface:
- `LeftSideBar.tsx` re-rendered as a flat folder list (nested by handle tree, org as visual section), word "channel" gone from UI strings.
- Single primary affordance per folder row: *Open* (wraps `sec launch`).
- Stamp dialog label changes from "Stamp" to "Approve" (or whichever word survives the Marcelo/Christophe check). Touch-ID ceremony and Rust path unchanged.
- Single top-row *Inbox* folder rolling up pending-approval items across all channel-dirs.
- Substrate vocabulary (channel, envelope, stamp, queue, DID) preserved everywhere it's load-bearing: MCP tool names, lexicon `$type`, CLI subcommands, AGENTS.md, on-disk file names. The folder UI is a *projection*, not a rename.

This pays off because the substrate already supports it — the v0.7 namespace collapse made channel-dirs uniform, the MCP-primary architecture (`project_mcp_is_primary_interface`) factored the UI to be a navigator, and `sec launch` already wires "open Claude in this folder." We're consolidating gestures the substrate already has, not building new substrate.

### No-gos
- No rename of MCP tools, lexicon types, CLI subcommands, or any wire-format string. The folder UI is a layman *projection* of the engineer substrate, not a replacement for it.
- No native Finder integration in this slice. App-hosted folder list only.
- No new substrate primitives (no "folder" record type — channel-dirs ARE folders already).
- No removal of `ReviewSurface.tsx` or `OrgPicker.tsx` — they stay as power-user surfaces; the layman entry point is additive, not destructive.
- No layman-facing creation flow for new channels — accepting invites and opening existing folders only. Channel creation stays in MCP / CLI (concierge surface) until the bet pays off.
- No cross-channel dashboards, no global search bar, no analytics. If the layman wants those, they're past the layman frame.
