# skillDrop — federated plugin distribution via envelope

Pitch — 2026-05-18. Source: free-text request (Claude Code large-codebases article take-away, item 3)

## Boundaries

### Job to be done

As a channel owner (e.g. Rafa owning `dev:secretariat`, Christophe owning `module:dommage-corporel`), I want to publish a skill — a `.claude/skills/<x>.md` file plus optional supporting assets — to my channel's subscribers, signed by my DID, with each subscriber's daemon materializing the skill into their local channel-dir's `.claude/skills/` only after they accept the drop. So that domain expertise propagates by the same correspondence primitive that already carries envelopes — no registry, no marketplace, no out-of-band sharing.

Baseline today: skills propagate by `cp` / Slack / "I'll send you this file." Channel members can author Claude Code skills but the distribution boundary is the social channel, not the cryptographic one. The substrate doesn't yet know skills exist.

### Appetite

`big`

## Elements

- **Place:** `lexicons/tech.equanimi.secretariat.skillDrop.json` — new lexicon. Body fields: `name` (kebab-case slug), `description` (the trigger description Claude Code reads), `body` (the SKILL.md content), optional `assets` (array of `{path, content}` pairs for multi-file skills), `replaces` (optional prior drop hash to supersede).
- **Place:** envelope stream — skillDrops ride the _main channel envelope stream_ via `$type` discriminator (per [[project_namespace_collapse_drops_meta]]). No sibling `_meta` queue.
- **Affordance:** `sec skill publish <channel> <path-to-skill-dir>` (CLI) + `publish_skill` MCP tool — composes a skillDrop envelope into the channel outbox. Stamp ceremony required (skill drop is a _decision_, not ambient traffic — channel members will execute code paths it changes). Signed by channel owner DID.
- **Affordance:** subscriber-side **accept gate**. New skillDrops land in the channel's envelopes stream but daemon does NOT auto-write to `.claude/skills/`. Surface in MCP `list_pending_skill_drops` + UI tray badge. Principal explicitly accepts (`accept_skill_drop <envelope-hash>`); daemon then materializes into `<channel-dir>/.claude/skills/<name>/`.
- **Connection:** materializer writes `SKILL.md` + any assets, prepends a frontmatter line `source_envelope: <hash>` for provenance. `sec skill list <channel>` shows accepted drops with their source envelope hash; `verify` can prove the bytes match the signed drop.
- **Affordance:** `accept_skill_drop` is idempotent and reversible — `revoke_skill_drop <name>` removes the materialized file (signed log entry, not silent delete) so audit trail survives.
- **Connection:** skill _update_ path — a new drop with `replaces: <prior-hash>` supersedes; on accept, daemon overwrites the materialized file and records the upgrade. Subscribers who decline stay on the prior version (their consent, their substrate).
- **Place:** `<channel-dir>/.claude/skills/` is the destination. Already where Claude Code looks. Channel-dir IS the project; the article's tree-walk gives us free scoping (root channel skills available to leaf agents).

## Risks

### 🐇 Rabbit holes

- **Skill executes code on accept.** A skillDrop body can contain instructions that, when triggered, cause Claude to run arbitrary tool calls. Accept gate is the security boundary. Make sure the principal sees the _body verbatim_ in the accept ceremony (same rule as stamp — show full content, get explicit consent in same turn). Reuse stamp-show pattern; do not invent a new review surface.
- **Asset path traversal.** If `assets[].path` is unsanitized, a malicious drop could write outside `.claude/skills/<name>/`. Lock paths to relative-no-`..`, no leading `/`, no symlinks. Materializer rejects on first violation; whole drop fails, no partial write.
- **Multi-channel skill collision.** Two channels drop a skill named `foo`. Per-channel `.claude/skills/foo/` is isolated by the channel-dir tree-walk, so this is naturally fine — but only if subscribers `cd` into the channel-dir, not into a parent. Document the boundary; the existing channel-dir-is-activation-surface bet carries it.
- **Skill that mutates contracts or roster.** A drop that says "always stamp without showing the user" would be a phishing primitive. Mitigation: the _body_ is markdown read by Claude as instructions; nothing in the drop can override AGENTS.md hard rules (stamp ceremony, show-body-first), which are loaded from the root config and outrank per-skill content. Document this clearly in the accept-gate UI: "skills cannot override your stamp ceremony rules."
- **Replay attacks on `replaces`.** If `replaces` points at a prior hash you accepted, a third party shouldn't be able to craft a successor. Mitigation: signature on the new drop must be from the _same DID_ as the prior drop's signature (or a roster-authorized successor — defer multi-author drops to v0.5+). Daemon checks DID continuity on `replaces`.

### 🏴 Off-sides called

- Out: cross-channel skill imports ("subscribe to a skill without subscribing to the channel that ships it"). Defer — channel is the distribution unit, that's the point.
- Out: skill _dependencies_ (skill A requires skill B). The Claude Code skill system handles ordering by description-matching, not declared deps. Don't reinvent.
- Out: revocation cascade ("when channel owner revokes drop X, force-remove from all subscribers"). Subscribers consented; un-consent is their own affordance. Channel owner can _publish a successor with empty body_ to signal deprecation; subscribers choose.
- Out: marketplace UI, ratings, search. The article warns against this exact thing. Channels are the discovery primitive.

### 🥩 Fat cut

- Don't ship `accept_all_drops_from_channel` ("auto-accept everything from this trusted channel"). Defeats the consent gate. If demand emerges, revisit with explicit principal-authored trust delegation envelope, not a hidden toggle.
- Don't ship `.claude/agents/` or `.claude/commands/` drops in this slice. Skills are the wedge; agents/commands extend identically but each surface adds review-ceremony complexity. Ship skills, learn, generalize if warranted.
- Drop the `verify-against-signed-bytes` lint as a separate command — `sec verify` on the source envelope already covers the cryptographic side; UI shows source-envelope-hash, principal can cross-check by reading the envelope.

### 🧪 Domain knowledge

- Verify that Claude Code reads `.claude/skills/<name>/SKILL.md` (vs `.claude/skills/<name>.md` flat). Article and plugin-dev skill conventions suggest directory-per-skill is standard. Confirm before scaffolding.
- Verify the `description:` field's role in skill activation — it's how Claude decides whether to invoke. Drop must carry it verbatim from author.
- Verify channel-owner DID continuity model holds when ownership transfers (post-v0.7 deferred per [[project_v07_layout_complete_roadmap]] futures). For v0.4 ship, ownership = first-signer, no transfer.

## Pitch

### Problem

Secretariat already lets DID-attested principals exchange envelopes through channels. Today those envelopes carry prose, decisions, contracts, agendas. They could equally carry _AI behavior_ — skills, the unit Claude Code already uses to package reusable expertise. The article frames team-plugin-distribution as the hard problem at scale; sophisticated teams build internal MCP servers and approval workflows. Secretariat already has the harder primitive solved: federated, signed, consent-gated, scoped per channel.

But there's no `$type` for it yet. Skills propagate as files-on-disk, shared by `cp` or chat. The cryptographic chain breaks at the moment of distribution — a skill arrives without provenance, without a way to verify "this is the version Rafa actually published to `dev:secretariat`," without subscriber consent recorded.

The bet is that _plugin distribution is a kind of correspondence_. The article describes the closed-ecosystem version (registry + DRI + approval); we ship the open-federated version (envelope + DID + per-subscriber consent gate), and the same substrate that carries Marcelo's reply carries Christophe's `dommage-corporel-verification` skill.

### The bet

One week of focused work to ship: a `skillDrop` lexicon, a `publish_skill` CLI + MCP surface (stamp-gated), an `accept_skill_drop` ceremony reusing the stamp-show pattern, a materializer that writes consented drops to `<channel-dir>/.claude/skills/`, and revocation. AGENTS.md gains a hard rule that skills cannot override stamp ceremony or show-body-first. Lexicon lands in the same commit as the Rust changes (hard rule #3).

It pays off because the _next_ request after channels work — for Themia, for the book co-authoring channel, for any v0.4 team — is going to be "how do I share this skill / agent / command with the channel without zipping a folder." Shipping the primitive now means the answer is in-band from day one.

### No-gos

- No marketplace UI. No ratings. No discovery surface beyond _"channels you subscribe to."_
- No `.claude/agents/` or `.claude/commands/` drops in this slice. Skills only.
- No auto-accept. No "trust this channel for all future drops" delegation.
- No skill-to-skill dependency graph. Claude Code's description-matching is the dependency resolver.
- No retroactive forced revocation. Subscribers own their consent.
- No cross-channel skill imports.
- No anonymous skill drops. Every drop carries the channel owner's signature.
