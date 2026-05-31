---
migrated_from: equanimi.tech/project/secretariat/20260526T193202Z-vvbfkf.md
$signature:
  $type: tech.equanimi.secretariat.signature
  signer: did:key:z6MkpcX3mHt44yNEDPDWJic8ocJdagzERxx5u2Qh1dWcVRVN
  signerRole: agent
  docHash: sha256:213c2868343b8cc38f679f7b864c19a1d0f40965a63ef82c0800bebc5414bdc3
  signedAt: 2026-05-26T19:32:02.714Z
  signature: ed25519:Gk3uEM3ZQXrzoaLsjXnmKBf8b2waXN70qsdDJnuCXhwmJ6tP1SJsjmgqAUcpNEhBTnHbi7RX1IhTfB9PJPBTAA==
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:988355d708dff7f2cd573a510fbc6ba6b9c48da10ef0943b168c265e0c58715a
  docFilename: 20260526T193202Z-vvbfkf.md
  stampedAt: 2026-05-26T19:34:44.872791Z
  signature: ed25519:7UTfLVkuWflUfZ8siGELLaEd2QdlIYkprKP+kcjT34+05Nhy8aLGA58tANRfNRmgPyvPqVA/ja3R1QDcxiILCA==
---

# Bloat audit + framing recommit

Session 2026-05-26. Walked secretariat against its original framing ("SPF/DKIM/DMARC for AI compositions — provenance not attribution") to check for bloat and drift.

## Audit findings

**Kernel (clean, \~3k LOC):** lexicons identity/signature/stamp/envelope, DID resolvers (did:key + did:web), ed25519 signer, biometric gate, verify path. Recent pitches sharpen the primitive — drop-envelope-depth-urgency, stamp-comprehension-gate, drop-outbox, collapse-namespaces. Discipline present in shaping.

**Drift surfaces (33k LOC Rust + 16k LOC TS total):**

| Layer                                                                                                                        | LOC        | Drift signal                                    |
| ---------------------------------------------------------------------------------------------------------------------------- | ---------- | ----------------------------------------------- |
| `crates/relay/`                                                                                                              | 4255       | Own server with auth + queue + persist + routes |
| `application/channel_def_envelope`, `channels_ops`, `agent_manifest_ops`, `contract_ops`, `invite_ops`, `sync`, `federation` | \~4000     | Coordination substrate                          |
| `infrastructure/cognition/` + `ports/cognition/`                                                                             | \~2000     | Owns brain launch + routing                     |
| `crates/daemon/`                                                                                                             | 1365       | Continuous federation                           |
| `mcp/src/server.rs` monolith                                                                                                 | 2733       | 30+ tools                                       |
| Tray: sessions, explorer, markdown editor, preferences, onboarding                                                           | \~12000 TS | Tabbed messaging app                            |
| Lexicons beyond identity/signature/stamp/envelope                                                                            | 8 of 12    | Substrate vocab                                 |

README still narrates v0.3 stance ("no main window, no walker UI, no settings panes, no compose UI — anti-compulsion") while codebase has shipped sessions, ExplorerTree, MarkdownWindow, preferences panes, Onboarding wizard. Pivot is documented (`docs/ideas/2026-05-12-secretariat-as-autonomous-enterprise-substrate.md`) but README + AGENTS narrative debt remains.

## Three lanes considered

**A. Split.** Carve `secretariat-protocol` crate + lexicons + `sec verify`/`sec stamp`. Tag v1.0 on the kernel. Current app becomes `secretariat-app` v0.x against kernel. Two binaries, two release cadences.

**B. Hard reset to protocol.** Delete relay, daemon, channels, orgs, cognition launcher, most MCP tools, most UI. Distribution shape: brew CLI, npm verifier, WASM browser verifier, VS Code/GitHub Action verifiers. DKIM-shaped adoption (unilateral, transport-agnostic).

**C. Stay substrate.** Keep current direction. Accept LOC volume. Reposition.

## Call: lane C, with refinements

Settled framing — three layers:

* **Kernel:** provenance protocol for AI scribes. Separable, future-extractable. Lexicons identity/signature/stamp/envelope + DID + verify. Independent product candidate.

* **Product (architectural description):** stamping / correspondence substrate. Multi-principal scribe coordination.

* **Product (daily-driver tagline):** scribe's workstation. The motion = working alongside AI scribes, reviewing and stamping what matters.

* **Themia:** a deployment of secretariat. Not the substrate itself. Different layer.

Workstation is the tagline; stamping/correspondence is the mechanism. Compatible.

## Dropped framings (and why)

* **"SPF/DKIM/DMARC for AI compositions"** — accurate for the kernel, undersells the product layer. Misleads contributors about scope.

* **"OS for autonomous enterprises"** — overreach. OS metaphor doesn't fit (no process scheduling, no memory management). "Autonomous enterprise" leans on Marcelo's book framing rather than your own. Conflates kernel and product. Marketing-speak.

## What survives the recommitment

Under the new framing, these are load-bearing, not drift:

* Channels + orgs + contracts — filesystem semantics of the substrate

* Cognition layer — syscall surface to the scribe brain

* Onboarding — boot sequence

* Tray app review surface — primary daily shell

* Daemon — sync/federation init

## What still hurts under the new framing

Real bloat regardless of lane:

* `mcp/src/server.rs` 2733 LOC monolith → split per domain (channels.rs, orgs.rs, stamp.rs, verify.rs, agents.rs). Maintenance + LLM-context cost.

* `contextify_capture.rs` 640 LOC — kernel or userspace? Smells like a feature that grew.

* README + AGENTS.md narrative debt. Cheapest fix, highest signal-loss risk if delayed — contributors onboard against wrong mental model.

* Anti-compulsion stance vs SessionTabs + ExplorerTree — either v0.3 constraint dropped during pivot (say so), or current UI is over-built (cut back). Not both.

## Bloat review parked

Full review of remaining bloat to happen together later. Three angles to walk when ready:

1. Against new framing — what was load-bearing under "OS" but isn't under "workstation + protocol kernel" (relay scale, cognition launcher scope, MCP server breadth).
2. README + AGENTS.md narrative realignment to kernel/workstation split.
3. Kernel-extraction feasibility — can the protocol be carved into a separate crate cleanly? Forcing function for clarifying what's product vs. protocol.

## Next concrete moves

* Rewrite README + AGENTS.md against kernel-vs-product split. Drop SPF/DKIM analogy at top-level positioning; keep it inside the kernel docs. Drop "OS for autonomous enterprises."

* Reconcile "no central server" invariant with relay reality — restate as "no broker, no registry, no marketplace; relays are org-owned, not platform-owned."

* Decision doc declaring kernel vs userspace boundary. Lets userspace evolve without breaking kernel ABI.

* Beachhead test: Christophe's first 7 days on secretariat. If onboarding → stamping → reviewing doesn't hook by day 7, the workstation framing isn't earning its mass yet. User-feedback loop, not code question.

