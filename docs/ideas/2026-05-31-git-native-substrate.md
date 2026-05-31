# Git-native substrate — repos as channels, git as transport

**Date:** 2026-05-31
**Status:** proposal, converging (Rafa + Claude as scribe)
**Supersedes:** the channel-tree / federation / owner-as-sequencer model
**Extends:** `docs/ideas/2026-05-27-signet-protocol.md`
**Supersedes (partially):** the back half of `docs/plans/2026-05-24-substrate-for-themia-plan.md` — see "What this does to the active plan"

***

## ⚓ Premise

Collapse Secretariat to **one substrate**: repo-resident markdown, stamped
in-place by Signet, versioned and transported by git. Drop the
`~/.secretariat/` channel tree, federation/relay/daemon, outbox/inbox, and
owner-as-sequencer entirely.

The evidence forced the move: **0 of 179 envelopes ever federated.** The
federation layer is dead weight. The only act ever exercised is
stamp-a-markdown. So build for that and delete the rest.

Three layers, cleanly separated:

* **Signet** — the stamping protocol (identity, signature, attestation, verify). Already extracted. Cross-platform, MCP-first.

* **Secretariat** — narrows to a **markdown editor + PKM** over repos, plus the **review walker**.

* **git** — storage, transport, and the over-time axis.

## 🗺️ The reframe — git inherits every primitive

We're not rebuilding the federation layer. We're **deleting it and inheriting git's**.

| Old (build new rails — nobody rode them: 0/179) | New (ride the rails everyone already runs)                 |
| ----------------------------------------------- | ---------------------------------------------------------- |
| channel                                         | a repo; its `docs/` is the log                             |
| federation / relay / daemon                     | `git push` / `fetch` + a forge (GitHub/GitLab)             |
| owner-as-sequencer                              | git history = the sequence; merge = ordering               |
| outbox / inbox                                  | branches / PRs / remotes                                   |
| multi-party correspondence                      | fork + PR; repo collaborators                              |
| counter-stamp (AG procès-verbal)                | each party commits their own `$attestation`; merged via PR |

## Claude is the universal git client

The non-developer objection dissolves. Christophe (avocat), Marcelo, dad
never touch git. **Claude (Code / MCP) push/pulls/PRs on their behalf.**
"Distribute where people are" resolves to: people are *with Claude*, and
Claude lives on git. The human stamps; the scribe operates the transport.

## A repo is a self-contained unit

```
<repo>/                          ~/.signet/  (was ~/.secretariat/ — now SHRUNK to identity only)
  docs/      ← stamped markdown     identity/key   ← never in git (invariant #3)
  assets/    ← small committed      did · profile
  .claude/   ← prompt templates     repos.json     ← the registered-repo list
  .git/      ← transport + history  (that's it)
```

`cd <repo> && launch` → docs, prompt templates, identity, and history are
all in scope. Doc-bound prompt templates run on doc classes: *"shape this
idea"* on `docs/ideas/*.md`, *"implement this"* on a pitch, *"handle
customer request"* on an inbound doc. Half of this already exists —
`/idea`, `/shaping`, `/review`, `/decision` plus the `.claude/` tree-walk
inheritance. Git-native just collapses it onto repos.

## ✂️ Cut bloat completely

The radical simplification. Secretariat bundles a protocol + a
correspondence app + a thesis. Git-native deletes the entire middle.

```
DIES — the whole correspondence/federation apparatus
  daemon            RelayServer · RelayClient · OutboxWatcher · InboxWriter
                     MetaResolver · RoutingEngine · ScheduleTicker      → git is the transport
  channels/orgs     create_org · invite · accept_invite · list_channels → repos + git collaborators
  compose/capture   write-to-queue ceremony                            → just write a .md in a repo
  outbox/inbox/drafts/sent                                             → branches, commits, history
  contacts/peers/DM · contact_store                                    → repo access list
  contracts (consumption + governance)                                 → a thin review-config file, if anything
  relay-state · federation endpoints · delivered: frontmatter          → git push/pull

SURVIVES — the irreducible core (~25 verbs → ~6)
  Signet     init · agent · stamp · verify · read     (separate, cross-platform, MCP-first)
  Editor     markdown editor + PKM (links: in_reply_to / references)
  Review     the cross-repo walker  ← the new center of gravity
  launch     cd <repo> && <cognition>
  git        storage + transport + over-time
```

The daemon does not shrink — it **dies**. Scheduled journaling, if wanted,
becomes a thin cron that commits markdown; no resident subsystem.

## 🪜 Review over repos = the granularity ladder, literally

The altitude-aware review **is** the seven-rung attentional-granularity
ladder applied to the review surface (see `semantic-zoom`). The principal
descends gross→subtle, stops at any rung (anti-compulsion), and the seal
happens only at the floor.

| Altitude        | Rung              | Renders                                                            | Reader question        |
| --------------- | ----------------- | ------------------------------------------------------------------ | ---------------------- |
| **0 — highest** | `⚓ Handle`        | `⚓ 3 repos · 7 pending` — tray badge, one glyph                    | "Anything waiting?"    |
| **1**           | Sentence          | one line **per repo**: `themia · 1 PV awaiting stamp`              | "Where's the heat?"    |
| **2**           | Paragraph (TL;DR) | pick a repo → what changed since last review + why, per doc        | "What's in this repo?" |
| **3**           | Page              | pick a doc → headline + **git diff since last stamp** + docHash    | "Enough to decide?"    |
| **4 — floor**   | Full body         | verbatim body — the stamp ceremony (hard rule #4: show-body-first) | "I vouch for *this*."  |

Descent is always gross→subtle, never reverse. **The stamp lives at the
floor** — you only seal what you've read in full. The `docHash` is the
provenance anchor: every coarse card up the ladder traces back to the exact
bytes sealed at altitude 4.

### What the walker walks

```
~/.signet/repos.json   ← registered repo list (the substrate IS these repos)
  └─ per repo, per doc, derive state from git + Signet verify:
       NEW        untracked/uncommitted design doc
       UNSTAMPED  committed, no $attestation
       REVISED    git diff since stamped docHash → legit new version (re-stamp candidate)
       TAMPERED   docHash mismatch, no new commit → verify state ✗   (integrity alarm)
       REQUIRED   dir/repo policy says this class needs a seal (was requires_stamp)
```

`git` gives the **over-time** axis; the walker's roll-up gives the
**across-repos** axis. Both questions that opened this design — "track over
time" + "review over repos" — are the two axes of one walker.

This **evolves** what exists: `sec review` + `/review` are already one verb
(per the roundtable-unified-with-review note). Today it walks channels;
git-native repoints it at repos, and the altitude ladder is the rendering
discipline `semantic-zoom` supplies. `two-buttons-cadenced-reviews` and
`bubble-up-like-hey` are the surfacing mechanism at altitudes 0–1.

## Over-time tracking

Git = version history (free, diffable). Stamp chain = each `$attestation`
pins `(docHash, stampedAt)`. A design tracked over time = one doc, a
sequence of stamps, each sealing the version the principal vouched for at
that moment. Verify answers: *"this* *`docHash`* *was sealed on this date by
this DID."*

## The key split — keys never enter the repo (invariant #3)

The `workspace-registry` doc (2026-05-12) already drew the line and it still
holds: **repo = publishable substrate; home = just the keychain + DID.**
Signet reads identity from `~/.signet/`, stamps docs in the repo. Private
signing keys never ride `git`.

## PKM = the Zettelkasten lineage; links become central

The `zettelkasten-recontextification` capture named the one missing
primitive: **links** (`in_reply_to` / `references`). It was filed "v0.4+,
don't shape yet." In a PKM-first Secretariat it's no longer deferred — it
**is** the core feature. A stamped doc references another by path/docHash;
the graph of references is the knowledge base. Git gives the over-time;
links give the across.

## 🖼️ Assets

Git handles assets natively, three tiers:

* **small** (diagrams, screenshots) → committed in `assets/`, referenced by relative path

* **large** (video, datasets, PDFs) → `.gitignore` or **Git LFS** — referenced by path, content excluded from history

* a stamped doc may `$attestation` over text that *references* an asset; the asset's own hash can be pinned in frontmatter when integrity matters

"Support assets even if excluded for size" = commit-or-LFS-or-gitignore,
path-referenced, optional hash-pin.

## Home repo

One **personal-knowledge repo** (cross-cutting journals / captures /
therapy — private, maybe never pushed) + **N project repos** (designs live
with their code). `mkdir ~/knowledge && git init` is the whole ceremony.

## 🎯 Why this is the right move (leverage)

Meadows lens. The old design intervened at a **low** leverage point — build
new structure, new rules, hope for adoption. `0/179 federated` is the
verdict: nobody adopts new correspondence rails. The new design intervenes
at a **paradigm** point — *stop being a network, become a convention layer
on git.* Highest-leverage move available.

## The one tradeoff — encryption

Git transport is plaintext-in-repo or private-repo ACL — **not** E2E.
Invariant #4 ("transports see only signed ciphertext") breaks. **Accepted
for v1: the repo is the trust boundary.** Body-encryption (`age` / Signet)
is a later app-layer add, already out-of-scope per `signet-protocol.md`.

## What this does to the active plan

`docs/plans/2026-05-24-substrate-for-themia-plan.md` is the live, stamped
work. Git-native **splits it in two** — stop at the relay boundary:

```
SURVIVES (reuse — protocol/PKM layer)        DELETED BY GIT-NATIVE (transport layer)
  Move 1  agent keys + identity signing       Move 5  federation → daemon
  Move 1  stamp-in-place                       Move 5  relay endpoint-resolution chain
  Move 3a collapse namespaces                  Move 7  owner-as-sequencer walkthroughs
  Move 3b drop DM/peer/contacts                daemon  federate.rs, inbox federation
  Move 4  drop _drafts/sent                     delivered: <relay-seq> frontmatter
  Move 6  requires_stamp channel field
  3-state verify (clean/tampered/unstamped) ← keep; it's the over-time integrity check
```

Everything up to "stamp-in-place + tamper-detect" is the Signet+PKM core.
Everything past it (relay, sequencer, federation) git eats.

## Casualties / migration

* **179 envelopes:** `mv` into target repos, never delete (hard rule: envelopes-never-destroyed). Pre-flight `tar` snapshot + post-move count gate. Pick a home repo for cross-cutting journals/captures.

* **AGENTS.md:** invariants #1 (no central server), #6 (bilateral/local), #8 (channel-dir = activation surface), #9 (owner-as-sequencer) all rewritten — the **repo** is now the unit.

* **Drop** daemon, relay, outbox-watcher, inbox-writer.

## Relationship to Signet

Signet stays the portable stamp primitive (cross-platform, MCP-first) and
has **no opinion on storage layout**. This note is the **app-layer**
decision: the app's substrate is git + repos. This note supplies the layout
Signet declines to.

## 📚 Prior art

| Doc                                                         | What it contributed                                                                                 |
| ----------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `2026-05-12-workspace-registry-and-repo-local-substrate.md` | The proto-vision — repo-local substrate, keys-stay-home split. Kept the relay; git-native drops it. |
| `2026-05-12-end-state-substrate-monoslice.md`               | Repo-local discovery was always in the "end state."                                                 |
| `zettelkasten-recontextification.md`                        | The PKM lineage — atomic / addressable / linkable / topic-bound; the link primitive.                |
| `secretariat-as-company-os.md`                              | Stamped decisions permeate all agents; one MCP-queryable substrate.                                 |
| `2026-05-27-signet-protocol.md`                             | The protocol extraction this note's app layer sits on.                                              |
| `2026-05-24-substrate-for-themia-plan.md`                   | The active plan this note half-supersedes.                                                          |

## 🔮 Future extension — the Saperene knowledge-graph layer (deferred)

The legacy KB at `~/Developer/saperene` (Obsidian→Logseq, ~3,000 notes,
108K `[[wikilinks]]`, a hand-built `_type/*` ontology of ~800 entities)
becomes the **personal-knowledge home repo**, ingested *as the principal's
own words* — unstamped by default, elected-stamp for canonical notes. Two
graphs, layered: the **wikilink graph** (authoritative, in the files, free,
navigational) and a derived **temporal knowledge graph**
(`getzep/graphiti` under evaluation — Kuzu/FalkorDB embedded, local/BYOK
LLM, ships an MCP server, bi-temporal "facts invalidated not deleted").
The KG stays a **regenerable read-cache, never authoritative** (invariant
#8); episodes trace back to the markdown. Fits the sovereignty stack
(no server, no telemetry, local keys, pluggable cognition, MCP-first).
**Deferred** — wikilinks + tags give navigation day one; the graph is
enrichment, not a blocker. Captured in memory `project_saperene_name_and_corpus`.

## 🧵 Consolidated captures (2026-05-31)

Four same-direction ideas, captured separately during a Things triage, were folded into this spine (their standalone files removed — this doc is the single source):

* `docs-in-repos-filesystem` → **already the Premise** (repo-resident markdown, drop `~/.secretariat/`). No new content; it *is* this proposal.
* `use-signet` → **already covered** by the three-layer split (¶ Premise) and "Relationship to Signet". Signet is the stamping layer wired into the repo-resident editor.
* `symlink-to-repos` → **new, surfaced as Open Question #6** — the repo-registration/linking step doesn't exist yet and blocks repo-resident docs.
* `alfred-search` → **new, downstream capability** (below).

### Downstream once repo-resident: filesystem search (Alfred)

Once docs are plain markdown in repos on the normal filesystem, an **Alfred workflow can search all Secretariat docs from anywhere** on macOS — no bespoke index, just the filesystem + the `repos.json` root list. Small, falls out for free *after* the pivot; not a blocker.

## 🚧 Open questions

1. **Home repo layout** — one personal-knowledge repo, or a small set by life-area? Lean: one, subfoldered.
2. **One repo per relationship, or per project?** A DM-equivalent = a private 2-collaborator repo?
3. **Forge dependency** — GitHub-as-relay reintroduces a central party. Acceptable for reach, or insist on bare-git remotes for sovereignty? Lean: forge for reach, bare-git supported.
4. **Migration sequencing** — do the 179 envelopes move before or after `sec review` repoints at repos?
5. **What remains of** **`contract.local.md`** — does per-repo review cadence/policy survive as a small config, or fold into `repos.json`?
6. **Repo registration mechanics** — how does a repo enter the substrate? Symlink each repo into a known root, or register absolute paths in `repos.json`? The linking step is the missing prerequisite for repo-resident docs (folded from the `symlink-to-repos` capture). Lean: absolute paths in `repos.json`, no symlinks.

