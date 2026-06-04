# Conductor teardown — steal the good, leave the bad

*2026-06-04. UX review of Conductor (coding-agent orchestrator). Captured live
during setup + settings walkthrough. Lens: Secretariat's grain — anti-compulsion,
git-native substrate, stamp ceremony, least-privilege, the review-session model.*

> **Screenshots not persisted.** The originals arrived as drag-to-clipboard from
> macOS temp staging and never hit disk; temp dirs were already cleared by capture
> time. Each screen is described verbatim below — re-screenshot into
> `assets/2026-06-04-conductor/` if pixels are wanted later.

***

## Verdict table

| #   | Screen / behavior                                                     | What it does                                                                                                                                    | Verdict          | Why                                                                                                                                             |
| --- | --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **Setup: provider grid**                                              | GitHub / Claude Code / Codex / More-providers cards, each with green ✓ + plan line ("Claude Max plan", "ChatGPT login")                         | **STEAL**        | Auto-detects existing CLI auth, no re-login. Reads `~/.claude`, `gh auth status`. Confidence at a glance.                                       |
| 1   | **Setup: theme + completion sound + message-sending** all on one card | Light/Dark/System, Queue vs Steer, Chime                                                                                                        | partial          | Layout good; **sound = leave** (see below).                                                                                                     |
| 3   | **Providers → Claude Code**                                           | Provider/Plan/Org/Account table; "Run claude /login"; CLI vs API-key toggle; "Open in `~/.claude/settings.json`"                                | **STEAL**        | Surfaces the substrate file directly, offers the login command inline, lets user pick auth method. Honest about *where config lives*.           |
| 4   | **Git settings**                                                      | Branch-name prefix (GitHub username / Custom / None); rename workspace when branch named; delete branch on archive; archive on merge; automerge | **STEAL**        | Branch/PR lifecycle as first-class settings. "Rename workspace from first message" + "archive on merge" map cleanly to a doc-task lifecycle.    |
| 5   | **Open-in dropdown**                                                  | Finder / VS Code / Xcode / iTerm / Terminal / Copy path — workspace-wide, numbered hotkeys                                                      | **STEAL**        | Exactly the `sec launch` affordance, generalized. One target picker per workspace, keyboard-driven. (VS Code icon weak — cosmetic.)             |
| 6   | **Sync agent configs**                                                | Copy skills, slash commands, MCP servers between Claude Code & Codex                                                                            | **STEAL**        | Substrate portability. Mirrors invariant #4 (cognition is pluggable) — config travels with the principal, not the vendor.                       |
| 6   | **Auto-convert long text**                                            | Paste >5000 chars → text attachment                                                                                                             | **STEAL**        | Keeps the conversation/context clean; large pastes become artifacts, not inline noise. Good for the editor.                                     |
| 6   | **Follow-up: Steer vs Queue**                                         | Send mid-turn (steer) or queue after finish                                                                                                     | **STEAL**        | Mid-turn steering is the right default for an editor where the human is co-present.                                                             |
| —   | **Workspace = git worktree**                                          | Each workspace is its own worktree + branch; placeholder city names until renamed                                                               | **STEAL (core)** | The keystone idea. Worktree as the *default unit of work*, invisible to the user. Structural answer to "don't stash/checkout to separate work." |
| 6   | **Desktop notifications**                                             | "Get notified when AI finishes"                                                                                                                 | **LEAVE**        | Violates anti-compulsion + review-session model. Hard no.                                                                                       |
| 1,6 | **Completion sound** (Chime / Jingle / Test)                          | Plays when agent finishes                                                                                                                       | **LEAVE**        | Same. No push, no chime. The endpoint is the human re-entering, not the app pinging.                                                            |
| 6   | **"I'm not absolutely right, thank you very much"**                   | Toggle strips "You're absolutely right!" from replies                                                                                           | **LEAVE**        | Cute cope. Fix sycophancy at the source, not with a string-strip switch.                                                                        |
| —   | **Placeholder city names** (tokyo…)                                   | Random name until branch is named                                                                                                               | **LEAVE**        | Cute noise. Secretariat wants names *derived from the doc*, meaning-first.                                                                      |
| 1   | **App-management / Apple Events grant**                               | Asked up front; powers the open-in targets                                                                                                      | **CAUTION**      | Broad — can script any scriptable app. Grant lazily, per-app, revocable in Privacy → Automation. Least-privilege over convenience.              |
| —   | **First chat didn't work**                                            | (your note)                                                                                                                                     | note             | First-run reliability gap. Watch for it in our own onboarding — the Marcelo lesson.                                                             |

***

## The borrows that matter for Secretariat

Three, ranked:

1. **Open-in target picker for** **`sec launch`.** Today `sec launch` opens the
   configured cognition CLI at a repo `cwd`. Conductor's dropdown
   (Finder/editor/terminal/Copy-path, numbered) is the same shape generalized to
   *any* target. Cheap win, keyboard-first, fits the editor command palette.
   See \[\[document-as-workflow-node]].

2. **Worktree-per-task as the launch default.** `sec launch` could open into a
   *worktree* of the repo, not the live checkout — agent drafts in isolation, the
   stamped substrate stays clean, merge-back is explicit. Matches the global
   anti-`checkout`/`stash` rule structurally instead of by discipline. This is the
   bigger bet; needs its own spec.

3. **Provider/substrate honesty panel.** The Providers screen surfaces *where
   config lives* (`~/.claude/settings.json`), offers the login command inline, and
   names plan/org/account. Secretariat already treats the filesystem as
   authoritative (invariant #5) — a "your substrate, plainly shown" panel is on-grain.

**Reject loudly:** notifications, completion sounds, name-noise. They're the
compulsion layer Secretariat exists to *not* have. Conductor is a productivity
tool; Secretariat is a review-session instrument. The screens where they diverge
are exactly the screens worth diverging on. See \[\[two-buttons-cadenced-reviews]].

***

## Delegation idea vs Conductor — same problem, opposite gate

Comparing \[\[2026-06-03-delegation-is-a-sealable-decision]] against Conductor.
Both orchestrate coding agents over git repos, dispatch work in, get a branch/PR
out. The split is *what advances the pipeline*.

### Similar

| <br />          | Delegation gauntlet             | Conductor                                 |
| --------------- | ------------------------------- | ----------------------------------------- |
| Unit of work    | repo (via `[[repos]]` registry) | workspace = worktree                      |
| Pipeline        | idea → shaped → planned → PR    | workspace → branch → PR → merge → archive |
| Multi-provider  | claude / opencode / LM Studio   | Claude Code / Codex / Bedrock             |
| Parallel agents | subagents per stage             | worktree-isolated, concurrent             |
| Auth            | detect CLI                      | detect CLI                                |
| Endpoint        | PR                              | PR                                        |

Both are **staged pipelines toward a PR**, both make the repo the dispatch
target, both ride existing subscriptions. Conductor *is* the orchestration
mechanics the delegation idea needs — proven, shipping.

### Different — the crux

1. **The gate.** Conductor advances on **convenience/momentum**: automerge,
   archive-on-merge, "AI finished" chime, desktop notif. System pulls *you*.
   Delegation idea advances on the **human seal** — pull, not push. The critique
   in the delegation doc names the Conductor-shaped failure exactly:
   *"daemon-on-seal is push in a trenchcoat."* Secretariat is actively resisting
   becoming Conductor.

2. **Provenance.** Conductor has **no trust layer** — work is plain git commits.
   Delegation idea: every dispatch carries a **signature** (DID-keyed), the
   terminal decision carries a **stamp**. The work product knows who authorized it.

3. **What they agonize over.** Conductor doesn't ask "is dispatch a signature or a
   stamp" — it just runs. Secretariat's entire hard problem (seal inflation,
   sealing intentions vs artifacts, infinite gauntlet) exists *only because* it
   insists on the trust layer Conductor omits.

4. **Compulsion.** Conductor = engagement surface (notify/sound/automerge).
   Secretariat = anti-compulsion, review-session. Opposite poles.

### The one insight

> **Conductor is the delegation gauntlet with the seal removed and the compulsion
> layer added.**

It proves the *mechanics* work — steal them (worktree-per-task, provider
auto-detect, repo registry, launch picker). Its *gate* is exactly the anti-pattern
the delegation critique killed — reject it.

The critique's saving reframe is the bridge: **dispatch = signature (cheap, every
time), PR = stamp (sober, once).** Conductor's commits are unsigned and its merge
is automatic; Secretariat's dispatch is *signed* and its terminal PR is *stamped*.
Same pipeline, trust layer added, compulsion layer removed.

So Conductor answers the delegation idea's open question *"is the daemon necessary
or is it the trap?"* — **it's the trap.** Conductor built the daemon (automerge,
notify) and it works *as a productivity tool*. Secretariat's whole reason to exist
is that seal-as-terminus, never-trigger is a different thing.
