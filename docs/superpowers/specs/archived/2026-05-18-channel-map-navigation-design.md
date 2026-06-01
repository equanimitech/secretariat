---
title: Channel Map Navigation
date: 2026-05-18T00:00:00.000Z
status: draft
authors:
  - rafa
---

# Channel Map Navigation

## Summary

A spatial navigation surface for the Tauri shell that replaces the current per-org button list with a continuously-zoomable map. Built on React Flow (Pro license already held). Implements the AG (attentional granularity) principle fractally: every entity is a **Zone** that renders the same five AG layers at scale-appropriate fidelity.

Navigation feel is Google Maps / Google Earth — pan the world, zoom into an org, zoom into a channel, zoom into an envelope. "As above so below": same primitive at every scale.

## Motivation

Current Tauri main window shows orgs as buttons in a list. Does not scale, does not show structure, does not match the "walking through messages" mental model. As channel counts grow and sub-channels nest (handle grammar is colon-recursive — `dommage-corporel:paris-cohort:experiment-2026-04`), a list/button model collapses.

The map metaphor fits the substrate because:

- Handles ARE colon-trees (matches geographic address nesting).

- Envelope chains ARE temporal sequences (matches Western reading order).

- The AG principle ("gross → subtle, deepening pathway") IS semantic zoom.

- `[[project_channel_dir_is_activation_surface]]` — each channel-dir is already a Claude Code project; the map's "enter zone" gesture maps cleanly to `sec launch`.

## Primitives

### The Zone (fractal)

Every entity is a Zone: org, channel, sub-channel, envelope. Each Zone renders five AG layers at zoom-dependent fidelity:

1. **Headline** — name / handle / subject. Always visible.
2. **Lede** — one-sentence state (last stamped envelope, recent activity, first sentence of body).
3. **Salience** — pulse / glow / badge (unread, stamp pressure, contract breach).
4. **Body preview** — expands with zoom. For channels: sub-zones + envelope chain. For envelopes: excerpt.
5. **Go deeper** — affordance to enter (double-click / Enter key).

Layer 4 contains two child types:

- **Chain** — ordered envelope sequence (chronological).

- **Zones** — recursive set of child Zones (sub-channels).

Self-similar all the way down. World view, org view, channel view, sub-channel view are not different "tiers" — they are the same primitive rendered at different camera distances.

### The Chain

Horizontal time axis. The universal grounding of every Zone.

- Recent on right (configurable per locale).

- Scroll horizontally = lateral walk through time.

- Pinch / scroll-zoom _while pointer is over the chain_ = time density (six months across screen ↔ one day zoomed in). Pinch elsewhere on the viewport = camera zoom (the standard map gesture). The chain owns its own zoom axis to keep time-walk and camera-walk separable.

- Size = stamp gravity (stamped envelopes slightly bigger). Visualizes `[[project_stamp_is_selective_weight]]`.

- Glow = unaddressed / unread. Decay function is `f(arrival_age, read_status)` — fades on read AND fades over time independently. Age-decay dominates after 7-14 days so neglected Zones do not stay perpetually loud (would otherwise become an ambient-anxiety driver per BCT review).

- New envelope on arrival = soft pulse-in animation. Ambient, no popup, no notification (per `[[feedback_review_session_model]]`).

- Stable positions (time is stable).

### The Archipelago (root view)

Special top-level: orgs as islands on a shared world map. Personal home (`~/` — your `_self` queue-root) is tentatively a first-class island alongside orgs (subject to the open follow-up below). Pan freely; click an island → camera flies into that org.

No teleport — every transition is a continuous zoom. Spatial memory across orgs.

## Navigation

Continuous zoom with three soft inflection points (continuous opacity ramp, not discrete tier breakpoints — matches "let attention settle"):

| Zoom   | AG layers visible                  |
| ------ | ---------------------------------- |
| 0–35%  | Headline only (label)              |
| 35–65% | + Lede crossfades in               |
| 65–90% | + Body preview (chain + sub-zones) |
| 90%+   | + Full content                     |

Gesture vocabulary:

- **Hover** → ephemeral preview overlay (headline + lede near cursor).

- **Click** → pin Zone in sidebar. Camera does NOT move.

- **Double-click / Enter** → camera flies into Zone.

- **Esc** → zoom out one level.

- **Cmd + ↑ / ↓ / ← / →** → step to sibling at same depth.

- **Breadcrumb click** → jump directly to any ancestor.

- **Mini-map (corner, v0.4+)** → always shows current location at archipelago scale.

## Layout primitive

**V1 ships auto-tile** — sub-zones flow into available space around the chain (responsive grid). Pragmatic, fastest to ship, position not guaranteed stable across new arrivals.

**Behind a flag for experiment: branch-at-creation-timestamp** — sub-zones spawn visually from the chain at the `rosterUpdate` envelope timestamp where they were created. Narratively meaningful (you SEE when `paris-cohort` forked from `dommage-corporel`), time-anchored, stable. Try after core nav lands; measure whether narrative anchoring beats responsive-grid pragmatics.

## Visual idiom

**Topographic.** Cartographic feel: terrain background, organic district shapes for Zones, inked road for the chain.

- System fonts (per `[[feedback_infrastructure_not_typography]]`).

- Sparing color — color carries SALIENCE (stamp glow, urgency tag, contract breach), not decoration.

- Light background (cream-paper terrain feel) for utility-tool legibility over long sessions.

Reinforces "walking through messages" metaphor literally. Stays infrastructural (Google Maps is utility, not designed reading experience).

## Sidebar

Viewport splits horizontally: map (flex 1) + sidebar (fixed width, collapsible via `Cmd + \`).

Pinned-on-click: sidebar updates only on intent. Pan and hover do not change sidebar content.

### Layout per Zone type

Every sidebar follows the same fractal AG shape:

```
[Read-out]   governance + state — what IS
[Edit-out]   your consumption + your settings — what YOU DO HERE
[Drafts]     unsigned outbox content for this Zone (N waiting)
[Actions]    Launch · Dispatch · New envelope · merged skills tree
```

### Read-out (per Zone type)

- **Org** — name, DID, total channel count. Channels with new activity are surfaced via glow on the map (salience), NOT via an always-visible enumerated count in chrome — counts appear on hover/click only. Prevents the inbox-zero anti-pattern bleeding in through a backdoor count (per BCT review).

- **Channel** — handle, DID-URI, roster (read-only), governance policy (read-only — artifact policy, stamp-required flag).

- **Envelope** — subject, from, timestamp, stream tag, trust layers (signature / stamp / counter-stamps per three-layer trust model).

### Edit-out

`contract.local.md` inline editor (YAML + markdown, no modal). Edits flow through MCP (`set_channel_contract` / `set_org_contract` etc.) — preserves `[[project_mcp_is_primary_interface]]`.

Ambient-feature toggles live in `contract.local.md` per Zone (and inherit org → channel per the consumption-contract stack):

```yaml
ui:
  animation: on | off # pulse-in, fly-to easing
  glow: on | off # unread/unaddressed glow
  mini_map: on | off # corner mini-map widget
```

Defaults `on`. Per Holistic Control test ("no undisableable features"): every ambient affordance must be toggleable individually without breaking the rest of the UI.

Governance (`channel.md`, `channelDef`) is read-only in v0.3 with a "Propose change" affordance that drafts a structural envelope into the outbox. Mutations require the Stamp ceremony (next section). Defer inline governance editing to v0.4 when counter-stamp ceremony lands.

### Drafts panel

Per-Zone view of unsigned outbox content. Each draft surfaces with its Stamp action.

The Stamp action carries the existing ceremony per `AGENTS.md` rule #4: show body verbatim, principal consents, Touch ID gates, signed envelope hits the chain with the pulse-in animation.

### Actions

- **Launch** — `sec launch <handle>` opens the cognition substrate (Claude Code / configured CLI) with `cwd` set to the channel-bound directory.

- **Dispatch** — headless agent (future slice — see `docs/pitches/2026-05-13-launch-dispatch-root-path.md`).

- **New envelope** — drafts an envelope into outbox, opens the editor.

- **Skills tree** — surfaces the merged `.claude/skills/` + `.claude/commands/` tree visible at this Zone (tree-walk inheritance from org → channel-leaf).

## Commit model

Surfaces the git metaphor explicitly. The substrate already does this — naming makes it teachable.

| Git          | Secretariat                                |
| ------------ | ------------------------------------------ |
| Working copy | `outbox/<handle>/`                         |
| `git commit` | **Stamp** (Touch ID, biometric, atomic)    |
| `git push`   | Relay enqueue → recipient/subscriber queue |
| `git log`    | Channel chain                              |

**The map shows committed envelopes only.** Drafts have no signature, no place in the chain. They live in the sidebar's Drafts panel.

**Structural changes are envelopes too.** Channel rename, roster change, contract.md edit, channel create — all flow as `$type`-tagged envelopes through the chain via the same Stamp ceremony. No special UI path; same Drafts panel, same Stamp button, same ceremony. Per `[[project_namespace_collapse_drops_meta]]` — roster + channelDef + skillDrop ride the main envelope stream.

## Distillation (deferred)

Every Zone has distilled AG layers (`headline` / `lede` / `why_matters` / `pyramid`) generated by the local `CognitionPort`. Cached as derived data alongside ciphertext at `cache/<envelope-hash>.distilled.json`. Regenerable. Generated lazily on first read OR eagerly by the Secretariat agent (`[[project_secretariat_agent]]`) during digest pass.

Privacy: never leaves device unless principal's substrate is remote (their choice, their key — per architectural invariant #5).

Not blocking for the v0.3 channel-map slice. Headline = handle / subject suffices initially. Distillation primitive layers on later without changing the Zone shape — the slots are already in the design.

## Stack

- **React Flow Pro** (existing license — Layout Pro, expand-collapse tree, custom node renderer).

- **Layout**: `dagre` for initial deterministic auto-tile (lighter, well-supported by React Flow community examples); force-directed-with-frozen-anchors revisited in v2 if narrative-anchored layout warrants it.

- **Custom node component** — implements the fractal AG Zone, opacity ramp tied to viewport zoom level via `useStore`.

- **View transitions** for fly-to camera moves — React Flow's `fitView` + easing curve is the baseline. React 19 `<ViewTransition>` is a stretch (requires verifying support in the Tauri 2 webview's Chromium version); use as a polish layer if supported, not a baseline dependency.

- **MCP for all CRUD** — sidebar edits dispatch to `set_channel_contract`, `set_org_contract`, etc. UI never bypasses MCP per `[[project_mcp_is_primary_interface]]`.

## Out of scope (this slice)

- Hand-curated drag layout (auto-tile only in v1).

- Branch-at-creation-timestamp layout (behind flag, experiment after v1 lands).

- Pin-multiple side-by-side sidebar (compare two Zones).

- Governance editing (read-only with Propose-change drafts; inline edit deferred to v0.4).

- Distillation pipeline (deferred — see above).

- Mini-map corner widget (v0.4+).

- Stream tracks within chain (multi-line subway view) — deferred to v0.4+; v1 chain is single line.

## Open follow-ups

- Confirm exact React Flow Pro features in scope (Layout Pro, expand-collapse tree, others).

- Tree-walk inheritance reach: does Tauri shell read `.claude/skills/` directly or only through `sec launch`? If only through launch, the sidebar's Skills tree section is forwarded into the launched session, not enumerated in-app.

- Animation budget: pulse-in + fly-to transitions at 60fps on M1 baseline.

- Personal home (`~/` `_self` queue-root) on archipelago — first-class island next to orgs, or pinned chrome (always center, always visible)?

- Mobile / smaller-window behavior (likely out of scope for v0.3 desktop slice).

## Equanimitech alignment

Walked against all nine principles (per operational default #1):

| Principle                  | Status     | Notes                                                                                                                                                                                                                                                                               |
| -------------------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Local-First Ownership   | ✅ Strong  | Filesystem-authoritative; no central server                                                                                                                                                                                                                                         |
| 2. Holistic Control        | ✅ Strong  | Per-feature `ui:` toggles in `contract.local.md` (see Edit-out)                                                                                                                                                                                                                     |
| 3. Modification Rights     | ✅ Strong  | Open source; per-channel skills tree; pluggable cognition                                                                                                                                                                                                                           |
| 4. Peripheral Presence     | ✅ Strong  | No notifications; ambient pulse + glow; map is _gone to_, not demanding                                                                                                                                                                                                             |
| 5. Attentional Granularity | ✅ Maximal | Design IS this principle — fractal AG Zone, continuous opacity ramp                                                                                                                                                                                                                 |
| 6. Bounded Experiences     | ✅ Strong  | No infinite scroll; map/chain/sidebar all naturally bounded                                                                                                                                                                                                                         |
| 7. Strategic Friction      | ✅ Strong  | Stamp ceremony = canonical Non-reactivity friction (ES-16, Decoupling Model)                                                                                                                                                                                                        |
| 8. Fade-by-Design          | ⚠️ **N/A** | Secretariat is **substrate**, not behavior-change intervention. Honestly declined per operational default #7 (resist over-application). Adjacent claim that holds: per-action ceremony overhead decreases as skills/contracts mature, but the tool itself stays. Do NOT claim Fade. |
| 9. Downstream Allocation   | ✅ Strong  | Map = filesystem. No recommender. Chronological chain (Decoupling Model)                                                                                                                                                                                                            |

Four-layer diagnostic (Convivial / Holistic Production / Attentive Consumption / Calm Interface):

- Convivial Infrastructure: ✅ Strong (Sovereignty)
- Holistic Production: ⚠️ Adequate, not maximal — process visible, judgment required, modifiable; skill-building is implicit (fractal AG teaches by repetition), not explicit. Future iteration: opt-in "your correspondence patterns over time" affordance (Franklin-style transparent process). Deferred.
- Attentive Consumption: ✅ Strong by construction (Awareness + Equanimity layers)
- Calm Interface: ✅ Strong — graceful failure (filesystem survives any layer crash)

Anti-pattern check: gamified scores — none; wellbeing theater — N/A (substrate IS the product); Fade-as-obsolescence — N/A; indifference branded as equanimity — opposite (selective stamp encourages caring deliberately); manifesto-coated mediocrity — principles implemented substantively.

## Validation against project memory

- ✅ `[[project_mcp_is_primary_interface]]` — UI navigates, MCP handles CRUD. Sidebar edits flow through MCP tools.

- ✅ `[[project_channel_dir_is_activation_surface]]` — Launch / Dispatch use `cwd`-into-channel-dir; cognition substrate sees the tree natively.

- ✅ `[[project_no_read_receipts]]` — no "delivered / read / seen" status anywhere in the design.

- ✅ `[[feedback_review_session_model]]` — sync principal-initiated, no notifications, Stamp = approval = send.

- ✅ `[[project_filesystem_authoritative]]` — drafts are real `outbox/` files, no DB; map renders from filesystem walks (read-cache permitted for query speed, regenerable).

- ✅ `[[feedback_infrastructure_not_typography]]` — system fonts, infrastructural feel.

- ✅ `[[project_stamp_is_selective_weight]]` — chain shows all signed envelopes; stamped ones glow but signed-only stay visible.

- ✅ `[[project_queue_uri_grammar]]` — handle's colon-tree IS the geographic nesting; map walks the same grammar.

- ✅ `[[project_namespace_collapse_drops_meta]]` — structural envelopes ride main stream; no sibling meta queue surfaced as a separate island/zone.

- ✅ `[[project_owner_as_sequencer]]` — chain order is the owner's sequence; no cross-channel global ordering implied by the map.
