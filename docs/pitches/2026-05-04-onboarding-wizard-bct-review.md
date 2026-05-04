# Behavioral analysis — Onboarding wizard

Companion to `2026-05-04-onboarding-wizard.md`. Reviews the wizard against BCT/PDP frameworks; tests for compulsion-reducing design + principal sovereignty.

## Context

Two-screen onboarding wizard. Target behavior: *complete first-time setup of Secretariat (name + identity + Touch ID + optional peer connection) in under 30 seconds without external help.* Wedge audience: non-developer professionals — lawyers, authors, advisors. Calm-tech / equanimitech-aligned.

## Identified BCTs

### 4.1 Instruction on how to perform the behavior (Shaping knowledge)
**Present because:** the italic note in screen 1 — *"Your identity will be generated on this device. Touch ID will protect every signature."* — instructs the principal on what's about to happen and what protects it. No demonstration needed; the model is stated.
**MoA:** Knowledge.
**Related PDP:** Suggestion, Reduction.

### 1.4 Action planning (Goals & Planning)
**Present because:** the wizard sequences the principal through a fixed five-action plan (set_profile → init_identity → Touch ID test → silent MCP wire → optional connect). They don't choose the order.
**MoA:** Behavioral regulation, Goals.
**Related PDP:** Tunneling.

### 12.5 Adding objects to the environment (Antecedents)
**Present because:** the app itself, plus the resulting on-device identity, are durable objects added to the principal's environment. Once present, the channel exists without further setup.
**MoA:** Environmental context and resources.
**Related PDP:** Reduction.

### 8.7 Graded tasks (Repetition & Substitution)
**Present because:** screen 1 is a small commitment (type a name, click a button). Screen 2 is fully optional. The principal isn't asked to do anything they can't refuse.
**MoA:** Self-efficacy, Behavioral regulation.

## Identified PDPs

### Reduction (Primary Task)
**Definition:** *Reduce intricate target behavior into simple heuristic-based tasks.*
**Present because:** "set up correspondence" is decomposed into one form + one optional decision. Five backend operations happen behind a single button.
**Implements BCTs:** 4.1 Instruction, 1.4 Action planning.

### Tunneling (Primary Task)
**Definition:** *Guide the user through a process or experience leveraging an interactive tour.*
**Present because:** screen 1 → screen 2, no escape hatches that drop the user into an unconfigured state. Closing the app pre-completion routes back to the wizard on relaunch (idempotent re-entry).
**Implements BCTs:** 1.4 Action planning.

### Tailoring (Primary Task)
**Definition:** *Information should be relevant to the user's profile.*
**Present because:** the name field personalizes every subsequent surface (review header, contact list on the other side). The DID-derived avatar is also tailored.
**Implements BCTs:** none directly (tailoring is more upstream).

### Trustworthiness (System Credibility)
**Definition:** *Ensure the users trust you. All information you provide to them should be truthful, fair and unbiased. Their privacy and well-being are key.*
**Present because:** the italic note in screen 1 names the trust model up front ("identity generated on this device", "Touch ID protects every signature"). No dark patterns, no opt-out-buried-in-settings.
**Implements BCTs:** none directly.

### Real-world feel (System Credibility)
**Present because:** the Touch ID test signature in screen 1 isn't just a UI flourish — it actually exercises the biometric gate end-to-end. The principal feels their finger gating a real signature in the first 10 seconds.

## Mechanisms of Action summary

The wizard's MoA stack is small on purpose:
- **Knowledge** (4.1) — *what's happening*
- **Behavioral regulation** (1.4, 8.7) — *what comes next*
- **Self-efficacy** (8.7) — *I can do this*
- **Environmental context** (12.5) — *the channel now exists*

Notably absent: anything operating via *Norms*, *Social influences*, or *Reinforcement*. By design.

## What's deliberately absent

These are tempting BCTs/PDPs the wizard refuses. Each refusal is a design choice that aligns with `equanimitech_principles.md` red lines.

| Pattern | Refused because |
|---|---|
| **10.4 Social reward** ("🎉 Welcome to Secretariat!") | Equanimitech red line: no completion celebrations. The wizard ends; the app is open. |
| **10.10 Reward (outcome)** / **14.4 Rewarding completion** ("You're all set! 1/1 complete!") | Same. No streaks, no progress %, no completion score. |
| **6.2 Social comparison** ("Join 10,000 professionals…") | No engagement-based marketing. Wedge audience has zero interest in this signal. |
| **6.3 Information about others' approval** | Same. |
| **PDP Praise** ("Great job!") | Trains for external validation; equanimitech is fade-by-design. |
| **PDP Surveillance** (clipboard auto-read on screen 2) | Surveillance-y; explicit paste only. |
| **PDP Self-monitoring** (progress bar, step count visible) | Implies the wizard is something to *complete*. It's a 2-step ritual; counting steps gives them weight they don't have. |
| **PDP Competition** | Anti-tagline. *"Async generative communication for professionals"* — not a leaderboard category. |
| **8.3 Habit formation** | The wizard is one-time. We're not trying to make the user open it daily. |

## BCT-PDP relationships

The wizard's design pattern stack:

```
Reduction  →  4.1 Instruction + 1.4 Action planning + 12.5 Adding to env
Tunneling  →  1.4 Action planning
Tailoring  →  (presence layer — name, avatar, DID display)
Trustworthiness + Real-world feel  →  (credibility layer — explicit trust model + actual Touch ID exercise)
```

Two layers — a **doing layer** (Reduction/Tunneling implement Action planning + Instruction) and a **trust layer** (Trustworthiness/Real-world feel earn the right to ask for fingerprint and name). Both layers visible in the same 30 seconds.

## Considerations

### Step minimization

The user's explicit constraint was *"reduce as much as possible the number of steps."* Two screens is the floor for this JBTD because:

- **Identity creation must precede peer connection.** Claiming an invite needs a DID. So even a 1-screen design would have to either generate the identity silently (ambushing the user with a Touch ID prompt with no context) or split into two beats.
- **Name is required for the UI surface.** A nameless DID renders the review surface as "you: did:key:z6Mk…" — the very thing the user complained about that triggered this work.
- **Peer connection is genuinely optional.** Keeping it on the same screen as identity creation forces a decision the principal might not be ready to make ("I don't have an invite — can I close this?"). Separation respects the optional-ness.

A sharper design would be **0 screens** — generate everything silently, prompt for a name later when the principal needs to display it. Two reasons not to:

1. **Touch ID consent.** The first biometric prompt should be a known, deliberate ritual, not a surprise modal. Putting it inside an explicit "Set me up" button is consent.
2. **Naming yourself is naming the app.** The first textbox the principal types into shapes their relationship to the tool. Hiding it behind a delayed prompt cheapens the moment.

So: 2 screens is the equilibrium between *minimal friction* and *intentional ritual*.

### Calm-technology concerns

- **Touch ID test signature** — could feel like a magic trick if not contextualized. Solve with a one-line sub-affordance: *"signing a test message to verify Touch ID works on this Mac."*
- **DID-derived avatar** — must be visually distinct enough that two principals' avatars don't collide accidentally. HSL hue from hash of full DID, fixed S+L for legibility. Not a profile picture; a presence indicator.
- **"Set me up" copy** — slightly imperative. Alternatives: "Create my identity", "Let's go", "Start". The current copy is fine; flagged for the design pass.

### Suggested improvements

1. **Show the DID after generation** — a small `did:key:z6Mk…` label below the name field, with a copy button. Reinforces sovereignty (you can see your key, you can take it elsewhere) without making it the primary thing.
2. **One-line "What is this?" link** below screen 1's italic note. Routes to a simple about page, not a video. Available, not in the way.
3. **Avatar editable from screen 1** — small color block next to the name field, click to cycle through 6 deterministic palettes. Tiny affordance, high agency.

### Equanimitech alignment check

| Principle | Status |
|---|---|
| 1. Local-first ownership | ✓ DID generated on device; never sent over wire |
| 2. Holistic control | ✓ Each screen has a skip; closing app preserves partial state |
| 3. Modification rights | ✓ MIT license; profile.json is plain readable JSON |
| 4. Peripheral presence | ✓ Wizard appears once, by design; not a persistent surface |
| 5. Attentional granularity | ✓ Gross (name + identity together) → subtle (peer connection) |
| 6. Bounded experiences | ✓ 2 screens, defined end |
| 7. Strategic friction | ✓ Touch ID is friction (deliberate); name field is intentional path |
| 8. Fade-by-design | ✓ Wizard runs once. Principal never sees it again. |
| 9. Downstream allocation | ✓ Principal types their own name; principal pastes their own invite or skips |

No red-line violations. Ship it.

## References

BCT groupings primary: *Goals & Planning* (1.4), *Shaping Knowledge* (4.1), *Repetition & Substitution* (8.7), *Antecedents* (12.5).
PDP categories primary: *Primary Task* (Reduction, Tunneling, Tailoring), *System Credibility* (Trustworthiness, Real-world feel).
MoAs primary: Knowledge, Behavioral regulation, Self-efficacy, Environmental context.
Source pyramid: `equanimitech_principles.md` — all 9 layers checked, no violations.
