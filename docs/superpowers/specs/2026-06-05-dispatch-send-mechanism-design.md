---
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:3ee22b34030b81dbee3c7c0b7dbede99d9a700bb6ecbb1a975c0d0c07cf6e377
  docFilename: 2026-06-05-dispatch-send-mechanism-design.md
  stampedAt: 2026-06-05T18:35:18.646578Z
  signature: ed25519:QP7Nh2sE2t3hCr44ISELI0lYtuzU1elEqd6LtRoIhAA9BB9yeC0nSVczXYBa+ldKfxExl7q1gPiDCHf7QyonDQ==
---
# Dispatch / send mechanism — design

**Date:** 2026-06-05
**Status:** approved (brainstorm), pre-plan
**Branch:** docs

## Problem

The principal wants to send a document — or anything composed from it — out of
the Secretariat editor to an external channel, without leaving the app. Today
there is no send affordance at all.

## Insight

The scribe is already the transport adapter. A headless `claude -p` invocation
carries the full Slack MCP toolset (`slack_send_message`, etc. — verified
2026-06-05). So we do **not** build a transport in Rust. We add a UI affordance
that drives the scribe through a fixed compose → gate → send-verbatim flow.

This is **the dispatch/send mechanism**. Its first and only target is Slack via
Claude's Slack MCP. The mechanism itself is transport-blind; the Slack-ness
lives in one prompt template.

## Trust posture

Send = **dispatch** = the signature layer (every authored body is signed
automatically). Sending requires no stamp. The Touch-ID stamp gate is untouched
and cannot be reached from this path — even headless, the scribe cannot seal.
So an always-available send button cannot escalate into a silent attestation.

No record type is sent — prose over the wire, not a
`tech.equanimi.secretariat.*` record. Therefore **no `lexicons/` diff** and no
stop-the-line gate.

The lapsed transports-as-adapters invariant forbids only *trust-weakening*
transports. This one does not touch the trust model.

## Architecture (Tauri-only; no core, no lexicon)

```
Editor toolbar  [Send]  (current doc open)
 └─ DispatchComposer  (free-text: "send the summary to #legal")
     ├─ invoke('dispatch_compose', {docPath, target:'slack', instruction})
     │     └─ spawn scribe (claude_code_sdk bridge) · COMPOSE prompt
     │        → returns { channel, body }     — DOES NOT SEND
     ├─ UI renders body VERBATIM + channel + [Send] [Cancel]   ← the human gate
     └─ on Send: invoke('dispatch_send', {target:'slack', channel, body})
           └─ spawn scribe · SEND-VERBATIM prompt
              → returns { ok, permalink }  → toast
```

Two scribe spawns per dispatch (compose, then send). The human gate sits
between them. Cost ≈ seconds + tokens ×2 per send — acceptable for a deliberate
act.

## Components

| Unit | Responsibility | Depends on |
|------|----------------|------------|
| `src-tauri/src/commands/dispatch.rs` — `dispatch_compose`, `dispatch_send` | thin Tauri cmds: build prompt for `target`, spawn scribe, parse result | cognition bridge |
| `dispatch_prompts` (pure fns, same module or sibling) | build COMPOSE / SEND-VERBATIM prompt strings per `DispatchTarget` | nothing — **unit-tested** |
| `DispatchTarget` enum | today: `Slack` only (one variant — documents the seam) | — |
| `src/components/dispatch/DispatchComposer.tsx` + toolbar button | instruction input → confirm body → toast | `invoke`, command ctx |

### The two prompt templates (the real logic, Slack target)

- **COMPOSE** — *"You are the scribe. Read the markdown at `{doc_path}`. The
  principal wants to dispatch it to Slack per: «{instruction}». Compose the
  Slack message body and identify the target channel from the instruction. DO
  NOT send. Return JSON `{channel, body}`."*
- **SEND** — *"Send this EXACT text verbatim to Slack channel `{channel}` using
  `slack_send_message`. Do not edit, summarize, or add anything. Text:
  «{body}». Return the message permalink."*

## Error handling

- Scribe / MCP unreachable → toast "scribe couldn't reach Slack"; composer stays
  open.
- COMPOSE returns no channel → UI prompts the principal to name it.
- SEND fails → toast error; composed body preserved in the composer for retry.
- Bounded spawn timeout on both invocations.

## Testing

- **Pure:** the prompt-builder fns — covered by unit tests (deterministic string
  construction; per-target selection).
- **Live:** dogfood on a real doc, simmer first — compose-only, eyes on the body
  before any send. Spawning real agents in CI is too heavy; the live path is
  verified manually per project convention (infrastructure tests favour real
  integrations, not mocks).

## Scope

**Keystone slice (build now):**
- `dispatch.rs`: `dispatch_compose` + `dispatch_send` over the bridge.
- `DispatchTarget` enum (`Slack`).
- prompt-builder pure fns + unit tests.
- `DispatchComposer` + editor toolbar Send button.
- one dogfood compose→send on a real doc.

**Deferred (separate, gated):**
- channel autocomplete / picker (free-text parse for now).
- default-channel preference.
- `slack_schedule_message` / draft variants.
- **release ceremony** — the 8-manifest lockstep bump is the principal's call,
  not folded into this slice (keel).

## Seam note — generalize at cardinality 2

Named at the mechanism level (`dispatch_*`, `DispatchTarget`) per the
principal's framing: *"this is simply our send mechanism."* The enum has one
variant today. When a second real target earns its place (Gmail, Linear — both
already in the scribe's MCP surface), add a variant + a second prompt template;
the gate / spawn / verbatim flow does not move, and per-target templates keep
the scribe's behaviour deterministic. Do **not** pre-build speculative targets.
