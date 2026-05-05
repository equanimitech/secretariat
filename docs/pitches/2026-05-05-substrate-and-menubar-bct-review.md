# Behavioral analysis — Substrate + tray + MCP-primary plan

Companion to `docs/milestones/2026-05-05-substrate-and-menubar.md`.
Reviews the 6-slice plan against BCT, PDP, and the equanimitech pyramid.

## Context

The Tauri app surface reduces to **tray icon + quick-pane + onboarding popover (one-shot) + daemon + MCP wiring**. All correspondence operations (compose, stamp, review, settings) move to Claude (via MCP) or CLI. Target behavior: principals sustain async stamped correspondence for professional work without the tool demanding attention.

## Identified BCTs

### 4.1 Instruction on how to perform the behavior (Shaping knowledge)
**Present because:** the onboarding popover's italic note (inherited from the wizard component built earlier) names the trust model; the tray dot's color semantics (red = needs setup, amber = pending, green = clear) instruct the principal on what they're seeing without text; the right-click tray menu items label affordances directly ("Capture an idea…", "Sync now").
**MoA:** Knowledge.
**Related PDPs:** Reduction, Trustworthiness.

### 1.4 Action planning (Goals & Planning)
**Present because:** the slice sequencing produces a structured day for the principal — capture (any time, quick-pane) → review (chosen time, via Claude) → stamp (intentional, Touch ID). Each step has a fixed shape; the principal isn't asked to construct the workflow.
**MoA:** Behavioral regulation, Goals.
**Related PDP:** Tunneling (only inside the onboarding popover; broader workflow is implied not enforced).

### 8.7 Graded tasks (Repetition & Substitution)
**Present because:** the MCP review walker (slice 6) advances one envelope at a time, gating each on principal confirmation. The principal isn't shown a flat list of 17 things; they're handed one decision at a time. Self-efficacy preserved.
**MoA:** Self-efficacy, Behavioral regulation.

### 12.5 Adding objects to the environment (Antecedents)
**Present because:** the tray icon is a durable object added to the principal's macOS environment. Once present, it ambient-signals state without demanding focus. The substrate's local queues + outbox files are also durable artifacts.
**MoA:** Environmental context and resources.
**Related PDPs:** Reduction, Real-world feel.

## Notably absent BCTs (deliberate)

These are tempting and explicitly refused. Each refusal is a design decision aligned with equanimitech red lines.

| BCT | Where it would have appeared | Why refused |
|---|---|---|
| **10.4 Social reward** | "🎉 First envelope sent!" celebration | Equanimitech red line — no completion celebrations |
| **14.4 Rewarding completion of behavior** | Streak counter on review sessions | Same |
| **6.2 Social comparison** | "Avg professional sends 3 envelopes/week" | Off-positioning + anti-equanimity |
| **6.3 Information about others' approval** | "Marcelo found this envelope helpful" | Read receipts — explicitly rejected (`memory/project_no_read_receipts.md`) |
| **8.3 Habit formation** | "Stamp something every day to build the habit" | The tool fades with use, not with daily-prompt nagging |
| **2.1 Monitoring of behavior** | Dashboard of "envelopes sent this week" | Anti-self-monitoring; not the principal's job to track their own throughput |

## Identified PDPs

### Reduction (Primary Task)
**Definition:** *Reduce intricate target behavior into simple heuristic-based tasks.*
**Present because:**
- Quick-pane reduces "remember to tell dad something later" to: shortcut → type → Enter (~4 seconds, ~5 keystrokes).
- Onboarding popover compresses "generate keypair, derive DID, store profile, register relay, claim invite" into one-button-plus-paste.
- "Review my outbox" → Claude calls `list_review_queue` + `read_envelope` per item. The principal says yes/defer/archive. Five distinct backend ops collapsed into one conversation.
**Implements BCTs:** 4.1 Instruction, 1.4 Action planning, 12.5 Adding objects to environment.

### Tunneling (Primary Task)
**Definition:** *Guide the user through a process or experience leveraging an interactive tour.*
**Present because:** the onboarding popover sequences full-name → display-name → identity-generation → invite-paste/skip with no escape hatch into an unconfigured state. Each step has a single Continue path. Closing the app mid-popover routes back into it on relaunch (idempotent re-entry).
**Implements BCTs:** 1.4 Action planning.
**Note:** tunneling is *only* used inside the bounded onboarding ritual. The daily-use surface (tray + Claude) does NOT tunnel — daily ops are open-ended, principal-paced.

### Tailoring (Primary Task)
**Definition:** *Information should be relevant to the user's profile.*
**Present because:**
- The two-name profile (`full_name` for letters, `display_name` for UI) tailors output per audience: Christophe Marchand on stamps, Christophe in the avatar.
- "Cognition is pluggable" (AGENTS.md inv. 5) — the principal picks their AI substrate (Claude Code, ChatGPT, local LLM). The tool tailors to the principal's existing infrastructure rather than imposing a vendor.

### Trustworthiness (System Credibility)
**Definition:** *Information should be truthful, fair and unbiased. User privacy and well-being are key.*
**Present because:**
- Onboarding popover italic note: *"identity generated on this device, Touch ID protects every signature."*
- Settings (CLI/MCP) surfaces `secretariat_root` so the principal knows where their keys live.
- AGENTS.md invariants 1–4 (no central server, no telemetry, keys-on-device, transports as adapters) are *encoded into the architecture*, not just promised in copy.
- No read-receipts (asymmetric metadata) enforces that the relay operator (currently Rafa, eventually anyone self-hosting) can't be a back-channel for sender surveillance.

### Real-world feel (System Credibility)
**Present because:** the Touch ID prompt at stamp time exercises the actual Secure Enclave biometric. The DID is verifiably the public-key-derived identity (recipient checks it independently). The signed bundle the auto-updater pulls is Ed25519-verified end-to-end against the embedded pubkey. Nothing is stage-managed.

## Notably absent PDPs (deliberate)

| PDP (category) | Where it would have appeared | Why refused |
|---|---|---|
| **Reminders** (Dialogue) | macOS notifications when an envelope arrives | Anti-equanimity. Tray dot is *ambient signal*, not a reminder — principal goes to it; it never visits them. (Borderline — see "Calm technology concerns" below.) |
| **Self-monitoring** (Dialogue) | "You sent 4 envelopes this week" stats panel | No surface tracks throughput. Tray dot reflects current state, not history. |
| **Surveillance** (Dialogue) | Read receipts | Explicit no — `memory/project_no_read_receipts.md`. Reply IS the read receipt. |
| **Praise** (Dialogue) | "Great job stamping!" | Trains for external validation; equanimitech is fade-by-design. |
| **Rewards** (Dialogue) | XP, badges, levels | Ditto. |
| **Competition** (Social Support) | Leaderboard of fastest stampers | Anti-positioning entirely. |
| **Cooperation** (Social Support) | "Together you and Pai have exchanged 12 envelopes" | Subtler version of competition — still gamification. |
| **Normative influence** (Social Support) | "Most professionals use the morning routine" | Manufactures pressure absent in the actual tool. |

## Mechanisms of Action summary

The MoA stack is intentionally narrow:

- **Knowledge** (4.1) — the principal understands what each surface element means.
- **Behavioral regulation** (1.4, 8.7) — the workflow has a stable shape; the principal isn't reconstructing the protocol each time.
- **Self-efficacy** (8.7) — one envelope at a time means each decision is small.
- **Environmental context** (12.5) — the tool is *present* (tray icon) without being *demanding*.

Notably absent from the MoA stack:
- **Norms** — no peer-pressure mechanism is engaged.
- **Social influences** — the only social field is the principal ↔ each peer they correspond with; we don't bundle peers into a "community."
- **Reinforcement** — no reward schedule of any kind.
- **Identity** (in the BCT-cluster sense, not the cryptographic sense) — we don't manufacture an "I am a Secretariat power user" identity. The tool fades.

## Equanimitech pyramid check

### Layer 1 — Sovereignty (Foundation)

**1. Local-First Ownership** ✅
- Keys live in `~/.secretariat/key`. Profile in `~/.secretariat/profile.json`. Ideas pool in `~/.secretariat/queues/inbox/triage/`. Outbox + inbox files local. The deployed relay holds signed-encrypted bytes only, and even those are removable when the principal self-hosts.

**2. Holistic Control** ✅
- Daemon optional (LaunchAgent installable / removable via `sec daemon install` / `uninstall`).
- MCP optional (don't wire if you don't want Claude integration).
- Quick-pane shortcut configurable.
- Tray icon dismissible via "Quit Secretariat" — daemon keeps running.
- Each surface can be disabled without breaking the others.

**3. Modification Rights** ✅
- MIT license. Profile JSON + envelope markdown + ideas markdown all plain readable formats. Forkable.

### Layer 2 — Awareness (Practice)

**4. Peripheral Presence** ✅ (with one note)
- Tray icon is the canonical example of peripheral presence. Static color dot, lives in the menubar, doesn't move into the principal's focus.
- Onboarding popover is the only surface that demands attention — *and only once, by design*.
- No badges with numbers, no animations, no movement.
- **Note:** the tray dot's amber state is technically "demanding-something" — it tells the principal "you have things to review." Mitigation: it's color, not number; it's static, not animated; it doesn't scream. The principal can ignore amber for hours or days without consequence (no escalation, no reminder, no follow-up).

**5. Attentional Granularity** ✅ (with risk in slice 6)
- Tray dot color (gross — single bit of state) → tray right-click menu (mid — actions available) → Claude conversation about a specific envelope (fine — full body, full context). Three resolutions, each invoked by deepening principal attention.
- **Risk:** slice 6 ships MCP tools but doesn't *guarantee* Claude walks one envelope at a time. Claude's conversational behavior depends on its prompting. Mitigation: tool descriptions must instruct *show one at a time, never list all*. Worth a follow-up audit when slice 6 lands.

**6. Bounded Experiences** ✅
- Onboarding popover ends on completion → tray-only forever.
- Quick-pane closes on Enter or Esc → no second screen.
- Review walker (in Claude conversation) ends when queue is exhausted → tray dot transitions amber → green.
- Idea capture is one-shot (type → save → close).
- No infinite scroll. No autoplay. No bottomless feed.

### Layer 3 — Equanimity (Outcome)

**7. Strategic Friction** ✅
- Compulsive path (notifications, badges, "you have unread") → does not exist. Principal cannot be pulled by the tool.
- Intentional path (capture, review, stamp) → frictionless. Cmd+Shift+S is faster than opening any other app. "Review my outbox" in Claude is faster than scrolling an inbox UI. Touch ID is friction *only* at the moment of attestation, which IS the intentional act.

**8. Fade-by-Design** ✅
- The tool's visible footprint is small (tray icon, occasional quick-pane). The tool doesn't grow more affordances over time — the surface stays bounded.
- The principal's reliance on visual hand-holding decreases: muscle memory replaces clicking; "review my outbox" via Claude replaces interactive walkers.
- A graduated user is one who barely notices Secretariat is there until they want it. The tool succeeds by being forgettable.

**9. Downstream Allocation** ✅
- Principal decides what to capture, when to review, what to stamp, what to defer, what to archive. The system has zero algorithmic curation — `list_review_queue` returns the queue as-is, in file-timestamp order. No "recommended" envelopes, no priority inference.

## Equanimitech red lines — full audit

| Red line | Status | Note |
|---|---|---|
| No completion checkboxes / done animations / finish rewards | ✅ | Walker just ends; tray dot transitions; nothing celebrates. |
| No streak counts / streak shame / longest-streak leaderboards | ✅ | No tracking of throughput at all. |
| No completion percentages / progress bars against targets | ✅ | Walker shows "envelope X of N" *only if Claude includes it in conversation*. Could prohibit in tool descriptions. |
| No push notifications, email reminders, badges, red dots | ✅ | Tray dot is amber/green/red but it's *state*, not a pop-up notification. The macOS notification API is wired (template) but the Secretariat code never emits. |
| No modal alerts | ⚠️ **One tension** | The onboarding popover is shaped like a modal. Defensible because (a) it's an entry point not an interruption, (b) it appears only once, (c) it has a natural endpoint. But it's the closest thing to a red-line violation in the plan. Worth being honest about. |
| No algorithmic curation | ✅ | File-timestamp order. |
| No performance ranking, comparative scoring, or leaderboards | ✅ | |
| No dark patterns / forced updates / account-gated features | ✅ | Auto-update prompts before installing; nothing gated. |
| No advertising, no engagement-based revenue | ✅ | Distribution is local app, no SaaS, no ads. |

## BCT-PDP relationships

```
Reduction → 4.1 Instruction + 1.4 Action planning + 12.5 Adding to environment
Tunneling → 1.4 Action planning  (only in the bounded onboarding popover)
Tailoring → (presence layer — full_name + display_name + cognition substrate)
Trustworthiness + Real-world feel → (credibility layer — explicit trust model in copy + actual cryptographic verification)
```

Two layers, like the wizard review found:
- **Doing layer** (Reduction + Tunneling) implements 4.1 + 1.4 + 8.7
- **Trust layer** (Trustworthiness + Real-world feel) earns the right to ask for Touch ID

The new third layer the merged plan introduces:
- **Substrate layer** (12.5 Adding objects to environment + Tailoring) — the local-queue captures + the two-name profile + the pluggable-cognition stance. The tool *fits the principal's existing world* rather than asking them to fit it.

## Calm technology concerns

Three flagged for honest acknowledgment:

1. **Onboarding popover is modal-shaped.** Defensible — see red-lines audit above — but if a reasonable critic squints, it's a modal alert in the technical sense. Mitigation: keep it small (~400×500), keep it tray-anchored (not screen-center), keep it ONE-SHOT. If the popover ever needs to reappear post-onboarding, it stops being a bounded experience and starts being a window.

2. **Tray dot amber state is a soft "demand."** A principal who sees amber feels mild pull to act. Mitigation: it's static, color-only, and the tool never escalates. The principal can ignore amber for a week and nothing degrades. Claim: this is *signal*, not *pressure*. But the line is fuzzy and the principal's actual experience may bend toward pressure. Worth checking with Marcelo / Christophe after a week of use.

3. **Quick-pane could become a capture compulsion.** "Every thought → Cmd+Shift+S" is a possible failure mode where the tool replaces internal reflection with external storage. Mitigation: there's no consumption surface inside the app, so capturing without reviewing creates pressure to review (amber dot), which is review-session-bounded. Compulsion is plausible but self-limiting. Counter-mitigation: don't ship a "you have N captured ideas" stats surface anywhere.

## Considerations

### Potential improvements

- **MCP tool descriptions need behavioral guardrails** so Claude walks one-at-a-time, never lists all. Tool description for `list_review_queue` should include: *"intended to be paginated by Claude — show envelopes one at a time during review sessions; do not dump full list to the principal."* Without this, the walker behavior is up to whatever Claude decides; equanimitech alignment becomes accidental.
- **Tray dot state needs a "pause" or "do not signal" mode** for principals who go offline (vacation, retreat, focus week). Without it, the principal returning after a week sees red/amber and feels the pull. With it, the principal can mute the signal explicitly. CLI: `sec routine pause --until 2026-05-12`.
- **Onboarding popover should set expectations explicitly:** *"This is the only window Secretariat will ever show you. Daily use is the menubar icon and your AI assistant."* Sets the equanimitech contract up front.

### Alternative approaches

- **Replace tray dot with tray glyph variants** (filled / hollow / outline) for state changes. Less attention-grabbing than color shifts on small icons. Worth a designer pass.
- **Quick-pane as voice input** in v0.4+ — captures more thoughts at the cost of friction tradeoffs. Out of v0.3 scope per the plan.

### Questions

- Should the tray icon be *removable* (right-click → "Hide tray icon")? Daemon would keep running; principal could rely entirely on Claude + CLI. This fits Holistic Control but means the principal forgets the app exists. Lean: yes, allow it. Possibly slice 7.
- Does the onboarding popover handle the case where the principal cancels mid-flow? Current spec: closing app routes back into popover next launch (good). But what if the principal *quits* mid-popover? Same — popover reappears on next launch. Good. Worth testing.

## References

BCT groupings primary: *Goals & Planning* (1.4), *Shaping Knowledge* (4.1), *Repetition & Substitution* (8.7), *Antecedents* (12.5).
BCT groupings deliberately absent: *Reward and threat* (10.x), *Comparison of behavior* (6.x), *Self-belief* (15.x — except where 8.7 already covers self-efficacy).
PDP categories primary: *Primary Task* (Reduction, Tunneling, Tailoring), *System Credibility* (Trustworthiness, Real-world feel).
PDP categories deliberately absent: *Social Support* entirely (Cooperation, Competition, Recognition, Normative influence — all rejected).
MoAs primary: Knowledge, Behavioral regulation, Self-efficacy, Environmental context.

Equanimitech principles checked: all 9 (3 sovereignty, 3 awareness, 3 equanimity). One tension (modal-shaped onboarding popover) flagged but defensible. No red-line violations.

Verdict: **ship the plan as-is.** Three follow-up audits at slice landing time:

1. After slice 4 (lifecycle + popover): test the onboarding-popover-as-modal feel with Marcelo and a fresh Christophe install. If it feels demanding rather than welcoming, scale the shaping.
2. After slice 6 (MCP review tools): audit Claude's actual conversational behavior with the new tools. If Claude lists 17 envelopes flat, tighten tool descriptions until it walks one-at-a-time.
3. After two weeks of real Marcelo correspondence: check whether the amber tray dot creates pressure or just provides information. If the former, consider hollow-glyph variant or pause-mode addition.
