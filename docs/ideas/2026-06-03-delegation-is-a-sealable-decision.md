---
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:6d0ef5462c4c7cdae2cff31825189b9c024486101565493702b935c009fd3e1e
  docFilename: 2026-06-03-delegation-is-a-sealable-decision.md
  stampedAt: 2026-06-03T21:49:06.536402Z
  signature: ed25519:UBXTLWwMnjit4+heZ6xcnFKxuDHxyGhr8QwuBO6p7+Z/RRWucVsqExIs1xb8cp9p4KOwUsI8XRUqM34SNmAkBw==
---
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

## Stage 1 — critique (simmering, 2026-06-03)

First gauntlet run, on this idea itself. Critique agent's kill-shots:

* **Seal inflation.** "Seal *every* dispatch" contradicts "seal less but better." A stamp per stage trains reflexive Touch-ID — the seal becomes a turnstile token, killing the soberness that makes it mean anything.

* **Sealing intentions, not artifacts.** Hard rule #4 attests content read in full; a dispatch brief is a forward bet on work not yet done. Stamping it bends "I vouch for this record" → "I approve this action" — a speech act the lexicon has no shape for.

* **Daemon-on-seal is push in a trenchcoat.** Auto-dispatch on seal re-introduces system momentum (the no-push anti-pattern), and the gauntlet has no structural terminator — each stage manufactures the next stampable doc (infinite gauntlet).

**Reframe that may save it:** dispatch carries a *signature* (cheap, mandatory, provenance), only the *terminal* decision gets a *stamp* (selective, sober). Resolves the selective-stamp contradiction via the existing three-layer trust model. The gauntlet's PR is the one stamp.

Sharpest simmer questions:

* What's the unit of the seal — ever a readable artifact, or always forward-looking intention? If always intention, rule #4 may not apply — you'd need a *different* act, not the stamp.

* Is delegation a *signature*, not a *stamp*?

* Where's the gauntlet's structural terminator (why is PR the floor, not another stampable doc)?

* Is the daemon necessary, or is it the trap? Can the gauntlet advance by the principal *pulling* the next stage — seal as terminus, never trigger?

* Does seal frequency have a natural ceiling, or does it scale with throughput (and then it's already not sober)?

Don't shape yet.
