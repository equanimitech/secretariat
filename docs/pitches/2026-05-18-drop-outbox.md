# Drop the outbox — drafts live in the envelope stream

Pitch — 2026-05-18. Source: free-text — "review the pain for outboxes in secretariat, I don't think we should have them"

**Hard dependency:** v0.7.0 layout-complete is shipped (current `main`). This pitch presupposes the passport-rooted layout from `docs/decisions/2026-05-12-substrate-layout-v03.md` and the queue-dir alignment from `docs/pitches/2026-05-18-queue-dir-alignment.md`.

## Boundaries

### Job to be done

As Claude (scribe) and the daemon (sender), I want a single deterministic place where a draft envelope lives — both before and after the principal stamps it — so that the "is this ready to go on the wire?" question is answered by *state on the envelope* (signature present, draft flag absent), not by *which subdirectory the file sits in*.

**The *when*:** Claude finishes drafting a peer envelope (or an org-channel post). Today it writes to `<queue-dir>/outbox/<file>.md`; on stamp, the daemon moves it to `<queue-dir>/outbox/sent/<file>.md`. Two moves, three states (draft / stamped-but-undelivered / sent), one extra subtree, and a daemon watcher whose entire purpose is to notice files appearing in `outbox/`.

**The baseline:** today the principal has three signals to track — `outbox/` (waiting for stamp), `outbox/sent/` (post-delivery), `envelopes/YYYY/MM/DD/` (received). A draft to a single peer can live in *both* `outbox/` and `envelopes/` mental models depending on which surface (CLI `list`, MCP `review`, daemon log) is naming it.

### Appetite
`medium`

Picked because the conceptual change is small (collapse one subtree) but the surface area is wide — domain has no outbox concept already, but application (`compose_ops`, `review_queue`, `list_outbox_files`, `drain_outbox`), daemon (`outbox_watcher`), CLI (`compose`, `stamp`, `list`), and MCP (`compose` tool, review prompts) all name `outbox` in load-bearing ways. Override with `--appetite=big` if migration shape proves nastier than the in-place sketch below.

## Elements

Fat-marker sketch — five primary elements, no more.

- **Place:** the draft lives where the stamped envelope will live. For peer queues: `<peer-alias>/<handle-path>/envelopes/YYYY/MM/DD/<ts>-<hash>.md`. For org channels: `<orgs-root>/<org>/channels/<handle-path>/envelopes/YYYY/MM/DD/<ts>-<hash>.md`. No `outbox/` subdir, no `sent/` subdir. One tree.

- **Affordance — draft marker:** unsigned drafts carry a `.draft.md` filename suffix; the file is renamed to `.md` (atomic) at stamp time. Filename suffix beats frontmatter-only flag because (a) grep-from-shell is the principal's review modality, (b) `notify`-style watchers fire on rename events natively, (c) `ls` makes the state obvious without parsing markdown.

- **Affordance — review queue filter:** `list_review_queue` (currently `list_outbox_queue`) walks the envelope trees and returns files matching `*.draft.md`. The "unstamped + locally-captured" union that `review_queue.rs` already builds collapses to one walk.

- **Connection — daemon trigger:** `outbox_watcher` becomes `envelope_watcher`. It still watches the queue trees recursively but fires `drain` only on `.md` rename-from-`.draft.md` events (or fresh `.md` files whose siblings indicate no draft). Stamp ceremony's atomic rename *is* the wire-send signal. False-send prevention is the safety axis (see Risks).

- **Connection — compose contract:** `sec compose` and the MCP `compose` tool write to `<queue-dir>/envelopes/YYYY/MM/DD/<ts>-<hash>.draft.md` directly. No more "outbox" in tool descriptions, prompts, or return shapes — they speak in "draft envelopes."

## Risks

### 🐇 Rabbit holes

- **Daemon false-send on rename storms.** If the watcher fires on every `.md` it sees and a migration script renames 200 historical envelopes, the daemon thinks 200 new sends queued. Mitigation: trigger only on `notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both))` *from* a `.draft.md` source, not on raw `Create` of `.md`. Need to verify `notify` exposes the from-path cleanly on macOS FSEvents — the watcher today already filters; the new rule is one match deeper.
- **Stamp ceremony's atomic rename across volumes.** `~/.secretariat/` is one filesystem in practice, but if a future passport sits on an external disk and the daemon process renames cross-volume, atomicity is gone. Unknown how `KeyPaths` will handle multi-volume passports — punt by documenting "passport must be single-volume" as an invariant for now.
- **`.draft.md` files committed by accident.** If a channel-dir is a git repo (per the channel-dir-as-Claude-project rule), an accidental `git add .` ships draft envelopes upstream. Mitigation: every channel-dir's auto-generated `.gitignore` excludes `*.draft.md`. The migration touches `.gitignore` everywhere.
- **MCP `review_queue` prompt churn.** Tool descriptions and the `stamp.md` / `compose.md` prompt files have "outbox" baked into the agent-facing vocabulary. Search-and-replace risks soft semantic drift if any "outbox" referred to something else (it shouldn't, but verify before bulk-replacing).

### 🏴 Off-sides called

- **Don't touch ciphertext storage.** `_ciphertext/` and the wire encoding are out of scope — the wire envelope shape doesn't know about outboxes today; only the local filesystem layout does.
- **Don't redesign frontmatter.** The `stamped: false` ↔ `stamped: true` frontmatter flag (if present) can stay or be dropped, but it's not the load-bearing signal — the filename suffix is. Don't pull on the frontmatter thread in this pitch.
- **Don't unify peer drafts and org-channel drafts beyond what's already unified.** They share the same `envelopes/YYYY/MM/DD/` tree shape post-v0.7.0; that's enough.

### 🥩 Fat to cut

- **The `sent/` subdir entirely.** Today drafts are in `outbox/`, stamped envelopes move to `outbox/sent/`. Both go. Stamped envelopes go straight into `envelopes/YYYY/MM/DD/` — *the same tree that holds received envelopes*. Verify this doesn't break `list_inbox_files`'s "skip `sent/`" filter (`inbox_ops.rs:87,172`) — it'll get simpler, not more complex.
- **Per-recipient outbox subdir.** `crates/cli/src/commands/stamp.rs:107` mentions `outbox/<recipient>/`. Drop. Peer-alias dir + handle-path already addresses the recipient.

### 🧪 Domain knowledge

- **Visual grep ergonomics in a mixed tree.** Verify with the principal: does mixing draft + sent + received envelopes in one `YYYY/MM/DD/` directory hurt review? My read is no (filename suffix is loud, `ls` shows it, day-shard is small), but the principal grep-reviews and may disagree. Asking before building.
- **Watcher behavior on rename-back.** If a stamp fails halfway (Touch ID cancelled mid-rename), can the watcher see a phantom `.md` event and try to send a draft? Need to walk the stamp ceremony in slow motion against the new rules.
- **The `.draft.md` choice vs a hidden-file convention** (`.<id>.md` → `<id>.md` on stamp). Hidden-file fits Unix tradition for "not for normal listing" but breaks fat-marker grep + makes the principal's `ls` review weird. `.draft.md` suffix is more honest.

## Pitch

### Problem

Today, "is this envelope ready to send?" is encoded as a *location* — `outbox/` means draft, `outbox/sent/` means delivered, `envelopes/` means received. Three locations, two daemon moves, one watcher whose existence is justified solely by the location-as-state choice. The state model is on the filesystem, not on the envelope.

The cost shows up in five places that all touched in v0.5–v0.7 work: `outbox_watcher.rs` exists at all; `list_outbox_files` is a sibling of `list_inbox_files` instead of one walk; `review_queue.rs` carries explicit comments distinguishing "outbox files" from "review queue" because the file location overloads three meanings; MCP tool descriptions and stamp prompt drift toward "outbox vocabulary" instead of "draft vocabulary"; and the v0.7.0 migration had to special-case empty-`outbox/`-dir cleanup (commit `4d3daeb`). The recent migration commits (`13df900`, `4d3daeb`) are evidence that the outbox subtree is load-bearing in ways the substrate decision doc (`2026-05-12-substrate-layout-v03.md`) didn't intend — that doc shows `outbox/` as a peer of `envelopes/`, but never says *why* drafts deserve a separate tree.

The answer is they don't. Drafts deserve a *marker* (so the daemon doesn't send them and the principal can find them), not a *tree*. A filename suffix carries the marker; the stamp ceremony's atomic rename flips it.

### The bet

Medium-appetite slice. Collapse `outbox/` and `outbox/sent/` into `envelopes/YYYY/MM/DD/`. Drafts get a `.draft.md` suffix; stamp atomically renames to `.md`. The daemon's `outbox_watcher` becomes `envelope_watcher` and triggers on the rename event only. Compose surfaces (CLI + MCP) write directly into the envelope tree as `.draft.md`. Migration script walks existing `outbox/` and `outbox/sent/` subtrees, moves files into `envelopes/YYYY/MM/DD/` (suffix-or-not based on stamp state), and removes the empty `outbox/` directories — leveraging the count-gated pattern already established by the v0.7.0 migrations.

Pays off because (a) one tree per queue, one filename convention, one watcher rule — the substrate gets simpler in the direction the v0.3 design always wanted; (b) the principal's mental model collapses from "what's in outbox?" + "what's in envelopes?" to "what's in this queue?"; (c) every MCP tool description and prompt that currently says "outbox" can speak in terms of "drafts" and "envelopes" — vocabulary that matches the domain concept (drafting → signing → sent) instead of the implementation concept (file moved between dirs).

### No-gos

- No wire-format change.
- No frontmatter schema change.
- No multi-volume passport support.
- No change to the `_ciphertext/` tree, the relay protocol, or signature/stamp record shape.
- No change to `inbox_ops`'s receive-side semantics beyond simplifying its "skip `sent/`" filter to nothing.
- No new dependency on `notify` features beyond what the current watcher already uses; if FSEvents on macOS doesn't cleanly expose rename-from-paths, fall back to "watch for `.md` Create + verify no sibling `.draft.md` with same stem exists" and ship that.
