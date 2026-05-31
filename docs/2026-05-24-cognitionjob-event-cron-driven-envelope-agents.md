---
migrated_from: equanimi.tech/project/secretariat/cognition/20260524T101128Z-bhbctd.md
---

# CognitionJob — event/cron driven envelope agents

Pitch — 2026-05-24. Source: claude-conversation distilling threads from `secretariat:dev:20260518T101119Z` (rolling agents list) + `secretariat:cognition:20260521T145239Z` (opencode/pluggable cognition) + therapy channel pattern.

## Pitch

### Problem

Three concrete use cases want envelope-scoped agency:

* Therapy review (built, bespoke, Rafa-only — Python in a channel `bin/` dir)

* Shaping incoming ideas into pitches (intentioned, not built — `secretariat:dev` capture)

* Triaging support traffic into bug + reply drafts (intentioned, not built — same capture)

Three workflows, three cognition adapters needed (LM Studio for therapy privacy, Anthropic for shaping/support), three triggers (cron for therapy, event for the others). Today only therapy exists, as a Python script that doesn't scale to Marcelo.

Without a substrate primitive, every new agent is a new Python file. The opencode envelope (`secretariat:cognition:20260521T145239Z`) and the agents capture (`secretariat:dev:20260518T101119Z`) point at the same gap from opposite ends: cognition is pluggable in design but only Claude Code is wired; agents are wanted but no substrate carries them.

### The bet

`CognitionJob` in channel `contract.md` (governance, shared with roster, signed by owner). Job runner in daemon. `CognitionPort` trait with Anthropic + LM Studio adapters. Loop guard via source-tag. Agents sign with agent DID, write to `envelopes/`, principal stamps subset.

Pays off when Marcelo adds a job by editing one frontmatter block — no Python, no `uv`, no model knowledge — and when therapy's `bin/review.py` retires in favor of `prompts/therapy-extraction.md` + a one-line contract entry. Same primitive both. Sovereignty over cognition (invariant #5) becomes operationally real, not just architecturally promised.

### No-gos

* No tool-use protocol. Single-shot text-in/text-out. Richer needs route through `dispatch`.

* No `AgentSupervisor` lifecycle.

* No automated stamping. Agents sign; principals stamp.

* No GUI authoring.

* No cross-queue write without explicit `output_queue:` in the job.

* No conflation with existing compose AI auto-fill — different surface, different lifecycle.

## Boundaries

### Job to be done

As a principal who wants envelope-scoped agency without window-juggling and without locking into one cognition vendor, I want to declare in a channel's contract that "when a new envelope arrives" (or "every Sunday at 6pm") fires a specific prompt against a specific cognition adapter and writes the result as a signed envelope into a target queue — so that shaping, support triage, and journal extraction all collapse to the same primitive and laypeople configure agents via markdown frontmatter alone.

_When_: a new envelope lands in `ideas` and I want it auto-shaped into a pitch; a complaint hits `support` and I want a bug + reply pre-drafted; Sunday 6pm rolls around and last week's journal entries should extract to therapy review.

Baseline today: therapy is a Rafa-bespoke Python script with `uv` deps and prompt literacy — doesn't scale to Marcelo. Shaping/support are pure intentions in `secretariat:dev:20260518T101119Z`. No substrate carries this.

### Appetite

`medium`

One contract field, one daemon subsystem, one port trait, one or two adapters, one envelope-authoring hook. Five surfaces, each narrow because the trait stays text-in/text-out. Override to `small` if the LM Studio adapter slips to a follow-up.

## Elements

* **Place:** channel `contract.md` (governance, shared with roster, signed by owner) gains `jobs: Vec<CognitionJob>`. New VO `CognitionJob { name, trigger, cognition, prompt, context, output_queue }`. Lives on the governance contract — not `.local.md` — because jobs are channel-defining behavior: roster members see "this channel auto-shapes on arrival" or "this channel runs a weekly digest" as a transparency property of the channel itself. Adapter selection by name (`cognition: anthropic`); actual credentials resolve via `preferences.toml` per-principal — keeps the governance contract fully shareable (no secrets ever land in `contract.md`). `.local.md` reserved for receiver-side automation on incoming traffic (different concern, not in this slice). Lexicon edits extend `tech.equanimi.secretariat.channelContract` in the same commit.

* **Affordance:** `CognitionPort` trait in `crates/core/src/ports/`. Single method: `async fn complete(&self, prompt: String) -> Result<String, CognitionError>`. Narrow on purpose. No tool-use, no streaming, no multi-turn. Implementors: `AnthropicAdapter` (BYOK, Messages API non-tool mode), `LmStudioAdapter` (HTTP to localhost OpenAI-compat endpoint). Ollama / Bedrock follow when concrete drivers appear.

* **Connection:** `JobRunner` subsystem inside daemon. Subscribes to two sources: `InboxWriter` events (`EnvelopeArrived(queue, hash)`) and `ScheduleTicker` cron ticks. Looks up the queue's contract, iterates `jobs[]`, fires matching triggers. Execution = load prompt + load context + `CognitionPort.complete()` + sign result with agent DID + write to target queue's `envelopes/YYYY/MM/DD/`. Only the channel owner's daemon executes (per invariant #9 owner-as-sequencer); other roster members read the declaration but don't fire. Fire-and-forget. Failures logged to `_self/inbox/triage`. Not `AgentSupervisor` — no process lifecycle, no restarts, no health checks.

* **Affordance:** declarative context loader. `context: [{kind, queue, last}]` loads N envelopes from named queues pre-flight, stitches into the prompt as a context block before `complete()`. No tool-use loop, no model-driven querying. Just static context-window-filling at fire time. Covers shaping's "dedup against existing pitches" without tools.

* **Affordance:** agent envelope authoring. Job results sign with agent DID (the scribe DID per `authorized_agents` already in the substrate), carry `source: cognition-job/<job-name>` provenance + `ag_source: ai` AG attribution. Lands in target queue signed-only (not stamped) per trust model rule #4. Principal review surface shows it like any other inbound. **Loop guard:** `on_arrival` jobs skip envelopes whose source matches `cognition-job/*`. Hard rule, not config.

## Risks

### 🐇 Rabbit holes

* **Anthropic API key handling.** First BYOK secret in substrate. Per invariant #3 keys never leave device — same discipline applies. Plan: `~/.secretariat/preferences.toml` under `[cognition.anthropic] api_key`, file mode 600, never committed, never sent on wire. Contracts reference adapters by name (`cognition: anthropic`) — never embed credentials. Verify `preferences.toml` loader doesn't serialize secrets in logs/dumps.

* **Cross-queue write permissions.** Job in queue A writes to queue B. Today any queue the principal owns is writable; principal's signing key gates everything. When agent DID authors, agent's roster-membership in target queue should gate. Roster model is v0.4 — for this slice, agents inherit principal write permissions. Document the gap.

* **Cron precision + clock drift.** `ScheduleTicker` fires on minute boundaries. Therapy migration fine (weekly). Sub-minute out of scope. Daemon restart mid-cron: skip missed ticks rather than catch-up flood. Document.

* **LM Studio adapter brittleness.** Therapy already hits this — small models refuse on sensitive content, max\_tokens cuts, model name drift. Adapter surfaces raw provider errors verbatim — no retries papering over real failures. Therapy script's existing error vocabulary is the reference.

* **Prompt template format.** Plain markdown with a `<!-- CONTEXT -->` injection marker. No templating engine on day one. Swap if complexity demands later.

* **Governance contract authoring UX.** `contract.md` is shared, signed by owner — editing it isn't the same casual surface as `.local.md`. For a single-principal channel (most early use) the friction is invisible; for multi-party channels, adding a job means a roster-visible governance change. Acceptable, but worth surfacing in the principal-facing flow.

### 🏴 Off-sides called

* Tool-use / agentic loop. Different primitive (`dispatch`). Two shapes, distinct.

* `AgentSupervisor` lifecycle. Fire-and-forget. Persistent agents separate.

* Routing engine. `on_arrival` is direct event subscription, not routing. Routing handles cross-queue multi-step delivery, deferred.

* `manual` trigger / principal-invoked cognition. That's `sec compose --cognition <prompt>` — different surface, different command. The existing compose AI auto-fill (`title`/`lede`/`summary`) is the closest primitive; extending it is a separate slice.

* Receiver-side automation in `.local.md`. Reserved for a future slice — different concern (private filtering of incoming) from channel-owner-declared agents (governance).

* GUI for job authoring. Markdown frontmatter only. Tauri navigator can deep-link read-only.

* Stamp automation. Hard no per rule #4. Agents sign; principals stamp.

### 🥩 Fat cut

* LM Studio adapter slips to v0.3.1 if Anthropic alone unblocks shaping + support. Therapy migration waits. Pro: tightens slice. Con: doesn't validate `CognitionPort` against two implementations, which is the trait's whole point.

* Cross-queue output. First slice constrains jobs to write into their declaring queue. Solves shaping (idea → shaped pitch in same channel), breaks support (complaint → bug elsewhere). Decide on first concrete wedge.

* Context loader. Could ship without `context: []` (prompt-only), add later. Therapy doesn't need it. Shaping does. Bench by first wedge.

### 🧪 Domain knowledge

* Confirm Anthropic Messages API rate-limit + auth shape via `context7` (`@anthropic-ai/sdk` or REST docs) before locking adapter.

* Confirm LM Studio `/v1/chat/completions` path (therapy uses it — should still hold).

* Confirm agent DID + private key already loadable by daemon at startup. Daemon needs key access to sign agent envelopes; verify keychain access from daemon process context.

* Confirm `InboxWriter` emits a hookable event today, or whether the event bus is new work. Probably new — daemon plan lists it but doesn't ship it. May expand appetite.

* Confirm `contract.md` is parsed today and how it relates to the existing `channel.md` (channel definition). If `contract.md` isn't yet a real artifact, this slice introduces it — meaningful since governance contract was specified in AGENTS.md rule #6 but the codebase ships only `contract.local.md` so far.

