# AI agents as peers — DIDs, authorization, A↔H + A↔A

Raw capture — 2026-05-05.

- "How do we handle AI agents being recipients too?"
- The substrate's `Recipient::Peer(Did)` already accommodates AI agents — DIDs identify *anyone*, human or otherwise. An agent with its own ed25519 keypair has a DID; addressing it is identical to addressing Marcelo. The substrate needs no new variant.
- What the substrate *doesn't* solve is the **authorization model**: under what circumstances can an agent act on behalf of a principal? Three sub-questions:
  - **Identity provenance.** Does an agent's DID stand alone (the agent owns its own key, like any peer), or is it a *sub-DID* derived from a principal's (e.g. `did:key:z6Mk…/agents/marcelo-scribe`)? Stand-alone keeps the substrate uniform; sub-DID encodes the trust relationship in the identifier itself.
  - **Delegation contract.** What document authorizes the agent to receive / send / stamp on behalf of? AGENTS.md invariant 5 says cognition is pluggable; this is the contractual form of that. Maps to the book's *Agent Contract* (Marcelo's *Autonomous Enterprise*, ch. 12) — the agent operates within an envelope of permitted actions.
  - **Stamp authority.** A human's stamp = "I attested." An agent's stamp would be = "my policy attested." Same cryptographic primitive, different trust semantics. Recipients of an agent-stamped envelope need to know it's agent-stamped to weigh accordingly.
- Two distinct uses of "AI as recipient" worth disentangling:
  1. **Cognition-as-tool** (already supported): Claude drafts, the principal stamps. The principal is the recipient of the *thinking*, but the envelope's `Peer(Did)` still points at a human. Agent has no identity in the wire format.
  2. **Agent-as-peer** (substrate-ready, auth-model-not): an agent has its own DID. It receives envelopes, processes them, replies. `Recipient::Peer(agent_did)` works without substrate change.
- Adjacent flows the auth model unlocks:
  - **A↔H reply.** Agent processes an inbound and writes back. Trust: recipient knows it's agent-authored.
  - **A↔A delegation.** Marcelo's scribe-agent can talk to Rafa's research-agent without either principal in the loop. Useful for routine coordination (scheduling, draft circulation) the principals don't want to handle directly.
  - **Agent reading a principal's queue.** The principal's local-queue captures (`inbox:triage`) become input to a scribe agent that drafts proposed letters. Maps to the scribe-background-journaling idea (`docs/ideas/scribe-background-journaling.md`).
- Composes with the channels/broadcast addressing (from `docs/ideas/channels-as-broadcast-feeds.md` + the `RemoteQueue` recipient variant captured in `memory/project_substrate_simplifications.md`):
  - An agent can publish a feed (their `RemoteQueue`).
  - An agent can subscribe to channels and process posts.
  - A↔A coordination via a shared channel = inverse-Slack: agents post stamped messages to a topic feed, all subscribers read.
- Questions:
  - Stand-alone DID per agent, or sub-DID rooted at the principal? Standalone is operationally simpler; sub-DID is verifiability simpler ("this agent is authorized by Rafa" is structural, not contractual).
  - Where does the agent's signing key live? On the principal's device (Secretariat daemon orchestrates) or on a separate process / machine (more independence)?
  - Does an agent's stamp need a different domain separation byte / wire field so recipients can distinguish? Probably yes — the threat model treats human-stamped and agent-stamped differently.
  - Does the relay need to know an envelope is agent-stamped? Probably no — relay still sees only signed ciphertext.
  - What does the principal see in their app when an agent-stamped envelope arrives? Different visual treatment (sender icon variant)? Or transparent (an agent's DID is just a peer)?
- Implications for v0.3:
  - **None require change to the substrate.** Agent identity is an additive future concern that fits inside the existing `Recipient::Peer(Did)` shape.
  - **What it argues against:** any substrate decision that assumes peers are humans. The variant names (`Peer`) are good — peer-agnostic. The only place "human" sneaks in is the stamp ceremony's biometric gate; that gate is a human-specific implementation, not a substrate property.
- Don't shape yet.
