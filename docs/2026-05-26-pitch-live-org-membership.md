---
migrated_from: equanimi.tech/project/secretariat/dev/20260526T171649Z-vu4seg.md
$signature:
  $type: tech.equanimi.secretariat.signature
  signer: did:key:z6MkpcX3mHt44yNEDPDWJic8ocJdagzERxx5u2Qh1dWcVRVN
  signerRole: agent
  docHash: sha256:310fa32eb54d5d6a8b8bffb70923a26211ec8e7d6556554e0c93fb455a090387
  signedAt: 2026-05-26T17:16:49.156Z
  signature: ed25519:XSXKe5sMRttz7E9FoBszLUmrfw/t5mqzkjS9IVzPTvQo4ghUzahvFwF5xbKcmcqimtaQFcWR8SjkQxxT+iBpCQ==
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:310fa32eb54d5d6a8b8bffb70923a26211ec8e7d6556554e0c93fb455a090387
  docFilename: 20260526T171649Z-vu4seg.md
  stampedAt: 2026-05-26T17:17:34.865Z
  signature: ed25519:OOSZiK6CX8plzLTKGLf597UbDX+JNNVxhr4b7f9p8tjWsxvJgC7g3hl8mtqdkA+Dn9UWZxgYxm3BuM9GuahnCw==
tag: pitch
appetite: small
status: draft
slice_id: A'
source: shaping-2026-05-26
supersedes:
  - v1
  - v2
  - v3
  - v4
  - v5
  - v6
  - v7
hard_dependency: v0.11.4 + dc4a3c3 + channelDef lexicon
---

# Pitch — Live org membership

**Bet:** Ship grant-by-intent + `channelDef` emit/ingest so one org invite gives Marcelo live access — new channels appear in his sidebar within seconds, no re-invite.

**Why it matters:** Substrate Slack-replacement requires this. v6's enumeration shipped substrate debt — membership-as-frozen-snapshot vs membership-as-live-relation.

***

## Boundaries

**JBTD:** As Rafa wanting Secretariat to replace Slack for Marcelo + his team, I want one org invite to mean "live participant" — not a snapshot frozen at grant time. Baseline: enumeration freezes membership at create; new channels need re-invite; membership reads as artifact, not relation.

**Out:** Private channels (future — needs `membershipGrant` lexicon design); GLAIUM ceremony (Slice B); `MembershipClaim` envelope; `orgDoc` fetch (channelDef-on-meta supersedes); `did:web` helper; Tauri UI (including Notion-style frontmatter rendering); relay-side roster gate; `listed` visibility class.

## Elements

* **Grant intent on the wire** (extend `OrgInviteContext`, `crates/core/src/application/invite_ops.rs:95`). Add `scope_intent: { org | subtree(handle) | channels(list) }`. `--channels '*'` → `org`; `--channels <handle>` → `subtree(handle)`; explicit list → `channels(list)`.

* **`<org>:_meta`** **queue convention** (handle-grammar reservation, no new lexicon). Reserved `:_meta` suffix per org. Carries top-level `channelDef` envelopes. Subtree channelDef envelopes ride their parent's queue.

* **`sec channels create`** **emits** **`channelDef`** (`crates/cli/src/commands/channels.rs` + MCP `create_channel`). Posts signed `{$type: channelDef, handle, parent, created_at}` to parent queue (or `<org>:_meta` if top-level).

* **Daemon ingest path for** **`channelDef`** (new hook in `crates/daemon/`). On receive: resolve parent; check receiver's org-level scope; if scope covers, auto-write `<root>/orgs/<alias>/channels/<handle-path>/membership.local.md`. Tombstones (`tombstoned: true`) remove derived memberships; preserve envelope history.

* **Eager bootstrap on claim** (`persist_org_membership`). Write 1 org-level `membership.local.md`. Walk `<org>:_meta` queue history; replay all `channelDef` envelopes through ingest path → derives N per-channel memberships. Sidebar populates on first connect.

* **One-shot backfill** (`sec migrate` step). Emit `channelDef` envelope for each existing channel into its parent queue (or `<org>:_meta`). Topological order: parents before children.

## Risks

**🐇 Rabbit holes:** Race — `channelDef` received before parent membership exists (buffer + retry on next poll). `:_meta` suffix collision with user-named handles (reserve grammar-side). Backfill ordering wrong → child arrives before parent in subscriber's history (topological emit on inviter side).

**🏴 Off-sides:** Private channels — designed but deferred. Needs `--visibility private` on create, `membershipGrant` lexicon for explicit grants, daemon ingest path that excludes private from auto-discovery. Bundled out of v8 to keep slice tight; ships when first concrete need surfaces. `orgDoc` fetch superseded by channelDef-on-meta.

**🧪 Domain knowledge:** Confirm `channelDef` lexicon has `parent`, `tombstoned` fields (`lexicons/tech.equanimi.secretariat.channelDef.json`). Confirm relay handle grammar supports `:_meta` suffix. Test 20-channel backfill → fresh claim → 20 memberships materialized.

## Acceptance

1. `sec invite create --org equanimi.tech --role collaborator --channels '*'` ships invite with `scope_intent: org`.
2. Marcelo claims; vault has 1 org-level `membership.local.md` + 20 derived per-channel memberships from eager bootstrap of `<equanimi.tech>:_meta` history.
3. Rafa runs `sec channels create project:newthing` → emits `channelDef` envelope to `<equanimi.tech>:_meta`.
4. Within Marcelo's next poll cycle, `project:newthing` membership exists on his disk; daemon polls it; sidebar updates without restart.
5. `sec channels delete project:oldthing` → emits tombstoned `channelDef` → Marcelo's daemon removes derived membership; envelope history preserved.

***

_Supersedes: v1–v7 drafts (archived 2026-05-26). Drafted by Claude (scribe)._
