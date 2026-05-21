# Secretariat as the operating system of a company

Raw capture — 2026-05-05.

- Secretariat is the operating system of a company.
- All decisions are stamped by the decision makers.
- Stamped decisions permeate all agents / employees of the company.
- MCP finds them easily — every agent queries the same substrate, citations resolve to attested envelopes.
- Decision = stamped envelope. Authority = whose key signed. Provenance = built in.
- Questions:
  - What's the access model? Every employee-agent reads the org's stamped corpus, but who can stamp?
  - Roles / delegation: does an employee-DID inherit stamping authority for a scope, or is everything routed through a principal?
  - Is the "company" a multi-principal contract (bilateral × N) or does it want a new primitive (multilateral)?
  - How do non-decision artifacts (drafts, deliberations) coexist with stamped decisions in the same MCP-queryable surface?
  - Does this collapse into "company = a queue/relay shared between principals" or does it need its own aggregate?
- Don't shape yet.

---

Riff — 2026-05-05: Secretariat as the orchestration layer for a business.

- Reframe from "OS" to "orchestration layer" — narrower, more honest. Not the whole runtime; the _coordination plane_ between principals, agents, and decisions.
- The company-as-business angle (vs OS): orchestration implies workflow, sequence, hand-off. Decisions trigger downstream agent work. Stamps gate progression.
- Substrate already supports this shape: `Recipient::Peer(Did)` for principal-to-principal, `Recipient::LocalQueue` for agent inboxes / hand-offs. Add agent-DIDs as recipients (cf. the prior idea on AI agents as peers) and the orchestration is just routing over the existing primitive.
- What "orchestration" earns over "OS":
  - **Workflow**: decision A unblocks agent task B (subscribed to a queue or a stamp tag).
  - **Hand-off**: completed agent work comes back as a draft envelope to the relevant principal for stamp.
  - **Audit**: every step is an envelope, signed by whoever owned it (principal or agent-DID under principal's delegation).
- Questions:
  - Is the orchestration logic _inside_ sec-mcp (a workflow engine) or _outside_ (each agent reads queues, decides what to do)? Strong prior: outside — substrate stays dumb, agents are smart. Less primitive, more product.
  - How do principals declare "when stamp X lands, fan out to agents Y/Z"? Is that a contract field, a queue subscription, or just out-of-band convention?
  - Does this need a new envelope type (`task`, `hand-off`) or do drafts + stamps cover it?
- Don't shape yet.
