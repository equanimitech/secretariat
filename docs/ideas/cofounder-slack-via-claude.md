# Cofounder Slack messages through Claude — what's the integration flow?

Raw capture — 2026-05-04.

- "my cofounder keeps sending me slack messages through Claude, I wonder how we might integrate this flow"
- Observation: real-world Claude-mediated correspondence is already happening, just over Slack. Cofounder's Claude → Slack DM → my Slack notification → me reading. No stamp, no provenance, no attentional bound respected.
- The substrate-level question: should Slack be a Secretariat _transport adapter_ (per AGENTS.md invariant #4)? The wire format is already designed to be adapter-agnostic — sender's daemon would post to Slack via the cofounder's own credentials, recipient's daemon would poll Slack via mine. Same encrypted-bytes-over-dumb-pipe model as the relay.
- Different threat model than email/relay: Slack workspace admins see metadata + content (unless we encrypt body, which then defeats Slack's UX of readable threads). Tradeoff worth naming explicitly.
- Adjacent angle: maybe Slack isn't a _transport_, it's a _surfacing channel_. The actual envelope still travels over the relay; Slack just gets a "you have new Secretariat mail from <cofounder>, click to open" nudge. Keeps Slack workspace from seeing content; preserves stamp/encryption integrity.
- Yet another angle: Claude on the _cofounder's side_ could write directly to my Secretariat outbox via MCP (federated MCP — my MCP server exposes a tool the cofounder's Claude can invoke after some bilateral handshake). Skips Slack entirely. Aligns with the "two AI staff serving two principals" framing.
- The frequency angle: cofounder/Slack messages are exactly the high-volume, low-formality flow Secretariat's depth/urgency/cadence framework was designed for. If they keep coming through Slack ungated, the substrate's equanimity invariant doesn't help.
- Questions:
  - Slack as transport adapter, or Slack as surfacing-only nudge channel?
  - Does the Slack adapter encrypt envelope body (kills thread-readability for non-Secretariat people in the channel) or pass plaintext (defeats invariant #4)?
  - If federated MCP is the answer, what's the trust model — does my MCP server accept tool calls from any peer, only from contacted ones, only from peers with a bilateral contract?
  - Bidirectional or one-way? Today flow is cofounder → Rafa via Slack. Symmetric requires me to reply through the same channel.
  - How does the cofounder's existing Slack-via-Claude flow change if we route through Secretariat? Does it just become "Claude, send a message to Rafa" → Claude calls Secretariat MCP → done? That's already the current MCP design; the Slack part is just where it surfaces on my side.
  - Privacy of the Slack workspace itself — if my Secretariat polls Slack DMs for me, the cofounder's Slack admin sees that polling. Acceptable? Different from email metadata leak how?
- Don't shape yet.
