# Delegation is a sealable decision

* We seal a decision **every time we send a job to an agent / workflow**. The dispatch *is* the stampable moment.

* The stamp gates the **brief going out** — pull-at-dispatch — not a notification after the fact.

* Resolves the orchestration-yes / notify-no boundary: Secretariat orchestrates agent delegations and the human seals the brief before it goes; it never pushes "something needs stamping." The act of delegating is the ceremony.

* Fits the existing stance: no notifications, no push, anti-compulsion. The seal is the natural gate at the moment of handing off work, so "what needs stamping" never has to be a queue or a stream — it's the dispatch itself.

* Cognition substrates in scope: claude / opencode / LM Studio (local). Dropped openclaw.

* Surfaced while orchestrating the editor-reader-redesign build — a live instance: orchestrate subagents, human seals the consequential briefs/decisions.

* Questions:

  * What's the unit that gets sealed — the brief/prompt, the plan task, the workflow spec, or the whole run?

  * High bar holds: do we seal *every* dispatch, or only consequential ones (selective stamp)? Tension with "seal less but better."

  * Does the sealed brief travel *with* the agent (provenance on the work product) — does the agent's output carry the principal's seal on its originating decision?

  * Where does this live — daemon `AgentSupervisor` (deferred `RoutingEngine`), `sec launch`, or a new dispatch verb?

  * Relationship to counter-stamp: multi-principal sign-off on a delegation?

## First delegation — the stamp-gated gauntlet

The shape Rafa wants. Each **stamp** is the gate that advances an idea down the pipeline; agents do the stage work, the human seals the transition:

1. **Critical feedback on ideas** — agents critique, let them *simmer*. No stamp to enter; this is the holding pool.

2. **On stamp → shaping** promising ideas (`/shaping`).

3. **On stamp → planning** the implementation plan, *with architectural review* (`/ddd`, code-architect).

4. **On stamp → PR** with the implemented decision.

So: idea → (critique / simmer) → ⊜ → shaped pitch → ⊜ → reviewed plan → ⊜ → PR. The seal between each stage *is* the delegation — sealing the decision to advance, and dispatching the next agent run.

Don't shape yet.
