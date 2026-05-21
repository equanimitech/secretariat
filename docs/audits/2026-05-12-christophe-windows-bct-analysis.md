# Behavioral analysis — Christophe onboarding (Windows, MCP-only)

Date: 2026-05-12. Sequel to `2026-05-04-onboarding-ux.md` (Marcelo first-attempt audit).

Subject: Christophe (French lawyer, Themia co-founder, Windows, 0 dev experience, intimate Claude Desktop knowledge). Second non-dev principal. Second shot at the non-dev path.

## Context

Christophe's behavioral profile is meaningfully different from Marcelo's:

- **Motivation:** intrinsically high. He's a co-founder with skin in the game; he doesn't need to be _sold_ on Secretariat — he needs to be _enabled_ to use it. Fogg's B = M·A·P collapses to _Ability + Prompt_. Motivation is given.
- **Existing competence:** intimate Claude Desktop knowledge. His mental model of "talk to Claude, Claude does things via tools" is mature. The MCP integration model is already native to how he thinks about Claude.
- **Friction surface:** Windows + 0 dev experience. Terminals, config files, JSON editing, DID strings are all out of bounds. Anything that requires explanation crosses the friction threshold.
- **Disposition:** lawyer. Formal, precise, values authority and verifiability. Sycophantic UI ("Yay! 🎉") will repel; sober legal-feel will land.
- **Time budget:** scarce. Setup must fit in a single Claude session, ≤10 minutes start to "first envelope sent."

The target system: MCP-only install on Windows, no Tauri app, no daemon, no tray. Claude Desktop is the entire UI. The substrate (`sec-mcp.exe`) lives behind the MCP wall; Christophe never sees a terminal.

## Sub-behaviors and analysis

### 1. Install (one-time, .msi double-click)

**BCTs present:**

- **Instruction on how to perform behavior (4.1)** — the .msi is itself the instruction; double-click is the only verb.
- **Restructuring physical environment (12.1)** — the install IS the environment restructure (writes `sec-mcp.exe` to Program Files, edits `claude_desktop_config.json`).
- **Reducing exposure to cues to perform unwanted behavior (12.3)** — no terminal opens, no PowerShell, no certificate warnings (if signed). Removes the cues that would trigger "this is a dev tool, not for me."

**PDPs present:**

- **Reduction (PT)** — collapses the multi-step Marcelo flow (Terminal install + Claude restart + MCP config) into one .msi click. Per Marcelo audit: _"It was very clunky. He didn't know if it was installed or not."_ — Reduction directly addresses this.
- **Tunneling (PT)** — installer is a predefined sequence (Welcome → License → Path → Install → Done); no branching, no choices the user couldn't answer.
- **Surface credibility (SC)** — the .msi must be **code-signed** with the Themia / EquanimiTech certificate. Unsigned = SmartScreen warning = "Windows protected your PC" red dialog = instant trust collapse for a lawyer.

**Mechanism of action:** _Beliefs about capabilities_. Christophe must form the belief "I can install this" _before_ clicking. A signed, polished installer signals "this is professional software" and removes the capability question.

**Fail mode:** SmartScreen / "untrusted publisher" dialog. For a lawyer, this is the entire trust gate. **Code signing is non-negotiable for Christophe.** Marcelo's audit identified this as L-severity on macOS (.pkg + notarization); same applies double on Windows where SmartScreen is more aggressive.

### 2. First-launch onboarding (entirely inside Claude Desktop chat)

**BCTs present:**

- **Action planning (1.4)** — Claude states the plan: "We'll set up your identity, connect you to Rafa, and send your first envelope. About 2 minutes."
- **Goal setting (behavior) (1.1)** — implicit: "you'll have sent your first stamped envelope by the end of this session."
- **Behavioral practice / rehearsal (8.1)** — the first envelope IS the rehearsal. Send-to-Rafa as the safe first target.
- **Instruction on how to perform behavior (4.1)** — Claude wizards each step: "Now I'll ask you for your full name (used on envelope signatures) and a display name (informal — used in UI). What should they be?"
- **Salience of consequences (5.2)** — when the first stamp ceremony fires, Claude must surface what's happening: "This stamps your envelope. Once stamped, it's cryptographically signed and immutable. Windows Hello will confirm it's you."

**PDPs present:**

- **Tunneling (PT)** — Claude leads through identity → contact-add → first envelope as a fixed sequence. No menu, no settings panel, no "configure later" deferrals.
- **Reduction (PT)** — each prompt asks one question with a default Claude can infer.
- **Social role (DI)** — Claude as the _scribe_ / _secrétaire_, not a chatty assistant. Frame matches the product name. _"Je vais vous aider à mettre en place votre Secrétariat."_
- **Trustworthiness (SC)** — Claude is explicit about what's happening at each step (key generated locally, never leaves device; invite URL claimed; relay registered). No magic.
- **Expertise (SC)** — Claude demonstrates competence by handling the technical details invisibly (DID derivation, key write, MCP registration) while explaining only what Christophe needs to decide.
- **Real-world feel (SC)** — _"Rafa Ballestiero"_ shows up by name when the invite is claimed, not as `did:web:rafa.equanimi.tech`. Real people, real names, cryptography hidden.

**Mechanism of action:** _Beliefs about capabilities_ + _Beliefs about consequences_. Each step builds confidence ("I just did that, OK") and reveals consequences ("I just signed something — Christophe knows what that means legally"). The tunneling drives momentum; the first envelope is the success-experience that locks in the belief "I can use this."

**Fail mode mapping to Marcelo's audit:**

| Marcelo failure              | Fix via BCT/PDP                                                                                                                                                                           |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| "Didn't know if installed"   | Tunneling: installer's last screen says "Open Claude. It will greet you." Claude's first message confirms: "Secretariat installed and connected."                                         |
| "No 'you're all set' moment" | Tunneling: explicit terminal state — _"Vous êtes prêt. Rafa peut maintenant vous écrire."_                                                                                                |
| "Three disconnected steps"   | Tunneling: single Claude conversation, no app-switching, no copy-paste between surfaces.                                                                                                  |
| "Sending is a hassle"        | Reduction + Suggestion: Claude says "Want to reply to Rafa? Tell me what to write."                                                                                                       |
| "No bidirectional contact"   | Action planning + Real-world feel: invite-claim returns Rafa's name to Christophe AND Christophe's name to Rafa. Both sides see "you're connected to <name>" without manual DID exchange. |

### 3. Daily check-in (pull-on-conversation, no push)

**BCTs present:**

- **Prompts / cues (7.1)** — but the _cue is Claude Desktop itself_. Opening Claude is the cue; no separate notification needed. This is a key inversion of the usual notification-driven model.
- **Habit formation (8.3)** — the habit is _"I open Claude, I check Secretariat."_ Doesn't require a new ritual; piggybacks on the existing one.
- **Behavioral substitution (8.7)** — Secretariat substitutes for email-checking-as-a-context-switch. The same conversation pane now handles correspondence.

**PDPs present:**

- **Reminders (DI)** — but only _inside the Claude conversation_. When Christophe opens Claude, Claude can ask "Want me to check for new envelopes?" — a soft prompt, not a push notification. Or, more aligned: Claude waits for him to ask; the pull is principal-initiated per `feedback_review_session_model`.
- **Reduction (PT)** — "any new envelopes?" → one query, all answered.

**Anti-patterns to AVOID (locked):**

- **Praise (DI)** — never "🎉 You have a new envelope!" or "Great job staying on top of your inbox!" Sycophancy in a lawyer's professional context = trust collapse.
- **Affect (DI)** — emotional flourishes. Stay sober.
- **Streaks / counts / leaderboards (BCT 10.4 family)** — explicitly out per AGENTS.md. No "you've sent 12 envelopes this week" surface, no badge for first PV, nothing measuring activity.
- **Push notifications** — out per `project_no_read_receipts` + anti-compulsion stance. Cadence floor for humans = 15-min poll equivalent; pull-on-conversation satisfies this implicitly.
- **Competition / Social comparison (SS)** — no "see what other Themia members are doing." Each principal's substrate is sovereign.

**Mechanism of action:** _Knowledge_ (Claude as the always-available knowledge layer) and _Memory, attention, and decision processes_ (the pull happens when Christophe is already attentive, not when he isn't). The substrate respects attention rather than fighting for it.

**Fail mode:** Christophe opens Claude, doesn't think to ask about envelopes, missed envelopes pile up. Mitigation: when Claude detects pending unread envelopes during ANY conversation (via MCP resource read on session start), it can softly surface: "By the way, you have 2 unread envelopes from Rafa." One-line, dismissible, no follow-up if ignored.

### 4. Compose + stamp ceremony (Windows Hello biometric)

**BCTs present:**

- **Salience of consequences (5.2)** — the stamp dialog itself, with Windows Hello + a quoted headline ("Stamp: 'Convocation AG ordinaire du 12 juin'"), is the salience-of-consequences mechanism in physical form. The biometric prompt makes the consequence (irrevocable signature) visceral.
- **Anticipated regret (5.3)** — Claude shows the FULL body verbatim before the stamp ceremony begins. The "you're about to sign this" moment lets Christophe imagine the consequence of stamping incorrectly.
- **Behavioral practice / rehearsal (8.1)** — first stamp is also a rehearsal. Subsequent stamps reuse the same muscle memory.
- **Credible source (9.1)** — Windows Hello is OS-level, vendor-signed, well-known. Borrowing Microsoft's biometric credibility for the stamp ceremony.

**PDPs present:**

- **Reduction (PT)** — stamp is a single biometric touch, no password, no second factor.
- **Tunneling (PT)** — fixed sequence: Claude shows body → Christophe confirms intent → Windows Hello dialog → stamped envelope written.
- **Trustworthiness (SC)** — the Windows Hello dialog's "reason" text must carry the document's first-line headline + short hash prefix, per AGENTS.md rule 4. Christophe can cross-check: "the dialog says I'm stamping the AG convocation; that's what Claude showed me; matches." No mismatch means no spoofing.
- **Verifiability (SC)** — after stamping, `sec verify` can show the signature is valid. Lawyer-grade: cryptographic provenance, externally checkable.
- **Surface credibility (SC)** — Windows Hello dialog is native OS chrome, not a custom UI. Borrows OS trust directly.

**Mechanism of action:** _Beliefs about consequences_ — the ceremony makes the legal weight visible. For a lawyer, this is the central UX. A stamp that _feels_ like a stamp is the entire product promise.

**Fail mode:** Windows Hello dialog text is generic ("Sign in to confirm"). If the headline + hash prefix can't be passed through, the cross-check breaks; principal can't verify they're stamping what they think they're stamping. **Verify Windows Hello API supports custom prompt strings before locking the design.** If not, fall back: display the headline + hash prefix in Claude immediately BEFORE the biometric prompt, with an explicit "the next dialog will ask for your fingerprint/face — confirm it's you."

### 5. Stamp-required PV drafting for Themia AG

**BCTs present:**

- **Action planning (1.4)** — the AG itself is the temporal anchor: "AG ordinaire annuelle de Themia, prévue le 12 juin 2026. À la fin de la réunion, Claude pourra rédiger le PV avec vous."
- **Goal setting (outcome) (1.3)** — outcome = a stamped PV in `assemblee_generale` channel, legally archivable.
- **Behavioral contract (1.8)** — the AG PV literally IS a behavioral contract; the BCT and the artifact converge.
- **Salience of consequences (5.2)** — the stamp ceremony for the PV is heavier than for ordinary correspondence; Claude can surface this: _"Ce PV est l'enregistrement légal de votre AG. Une fois tamponné, il est immuable et juridiquement opposable."_
- **Valued self-identity (13.4)** — Christophe-as-Themia-co-founder, stamping the company's authoritative governance record. The act reinforces identity.

**PDPs present:**

- **Authority (SC)** — the stamped PV cites French SAS statutes implicitly (procès-verbal d'assemblée générale is a regulated artifact). Authority is borrowed from corporate law.
- **Real-world feel (SC)** — names of attendees, real signatures, real legal weight.
- **Tunneling (PT)** — Claude walks the PV-drafting flow: convocation → ordre du jour → délibérations → résolutions → vote → signature. Standard French PV structure.
- **Suggestion (DI)** — Claude proposes the PV skeleton based on session notes; Christophe edits, validates, stamps.

**Counter-stamp note:** v0.3 single-stamp regime is a known gap. Christophe stamps as président de séance; co-attendees are listed inline. When v0.4 counter-stamp lands, those same PV envelopes accept additional stamps without invalidating the original. _This is the v0.4 forcing function_ — see [[project-assemblee-generale-channel]].

**Mechanism of action:** _Beliefs about consequences_ (legal weight) + _Social/professional role_ (président de séance) + _Valued self-identity_ (Themia co-founder). The PV ceremony bundles three reinforcement mechanisms.

**Fail mode:** Christophe drafts a PV, stamps it, then realizes a delibération is wrong. Envelopes are immutable. Need a clear _correction_ path: a new PV envelope titled "rectificatif de la PV du <date>" — separate envelope, also stamped, cross-referencing the original via envelope hash. Document this pattern in `assemblee_generale` channel's `CLAUDE.md`.

## Mechanisms of action — summary

The behavioral design for Christophe converges on four MoAs:

1. **Beliefs about capabilities** — every step must answer "I can do this." Driven by Reduction + Tunneling + Surface credibility.
2. **Beliefs about consequences** — every stamp must answer "I know what I'm doing has legal weight." Driven by Salience of consequences + Anticipated regret + Verifiability + Authority.
3. **Memory, attention, and decision processes** — never fight for attention; piggyback on the existing Claude habit. Driven by anti-Reminders / anti-Push design + pull-on-conversation.
4. **Social / professional identity** — stamps as president-of-meeting, scribe-of-record, co-founder. Driven by Valued self-identity + Social role + Real-world feel.

## BCT–PDP relationships

| BCT (what to target)                   | PDP (how to implement)                              | Where it lands                                      |
| -------------------------------------- | --------------------------------------------------- | --------------------------------------------------- |
| Instruction on how to perform behavior | Tunneling, Reduction, Social role                   | Onboarding wizard, install                          |
| Action planning                        | Tunneling, Suggestion                               | First-session wizard, PV drafting                   |
| Habit formation                        | (anti-)Reminders, Behavioral substitution           | Daily check-in (pull-on-conversation)               |
| Salience of consequences               | Trustworthiness, Verifiability, Surface credibility | Stamp ceremony (Windows Hello dialog with headline) |
| Anticipated regret                     | Tunneling (show body before stamp), Trustworthiness | Pre-stamp body display                              |
| Credible source                        | Authority, Real-world feel, Expertise               | Rafa as inviter, French SAS PV authority            |
| Valued self-identity                   | Authority, Social role                              | PV ceremony as président de séance                  |
| Restructuring environment              | Reduction                                           | The .msi install itself                             |

## Most likely fail modes (ranked)

1. **SmartScreen warning on unsigned .msi.** Trust collapse before install. **Mitigation: code-sign the installer.** Non-negotiable.
2. **Claude Desktop config write fails silently.** Christophe restarts Claude, MCP not registered, Claude has no idea Secretariat is installed. **Mitigation:** installer verifies write succeeded; first-launch check in Claude (via Secretariat MCP heartbeat); if MCP not loaded, Claude tells him "I don't see Secretariat connected — let's re-run the installer."
3. **Windows Hello can't carry custom prompt headline.** Stamp ceremony loses the cross-check that prevents spoofing. **Mitigation:** verify API capability up-front; fallback to Claude-side pre-prompt display if needed.
4. **Onboarding wizard goes off-script.** Christophe says something unexpected mid-wizard ("wait, can I rename my display name?"). Claude breaks tunneling. **Mitigation:** wizard skill in `.claude/skills/secretariat-onboarding/` that recovers from detours and tunnels back.
5. **First envelope to Rafa succeeds, then 5 days of silence.** Habit doesn't form. **Mitigation:** ensure Rafa replies within 24h to the first envelope; the _response_ is the reinforcement that locks in the habit. (BCT 8.1 + 8.3 work via experience, not instruction.)
6. **Christophe drafts PV, scrolls past body, stamps blindly.** Trust ceremony fails — he didn't actually read what he signed. **Mitigation:** Claude pauses after rendering body and explicitly asks for confirmation in the same turn before initiating stamp. AGENTS.md rule 4 already encodes this; surface it specifically for PV ceremonies.
7. **Update breaks MCP config.** Auto-update changes path; Claude Desktop config still points to old binary. **Mitigation:** updater rewrites Claude Desktop config on install. Verify version-skew handling. (Marcelo audit identified this; same fix applies.)

## Minimum-friction install + first-week behavior

**Install (≤2 minutes):**

1. iMessage / Themia channel from Rafa: _"Salut Christophe, voici Secretariat : [signed-msi-url]. Après installation, ouvre Claude Desktop, je te guide."_ + invite URL.
2. Christophe clicks .msi → Tunneling installer → Done screen says _"Ouvrez Claude Desktop pour commencer."_
3. Claude Desktop launches; on first message Claude greets: _"Bonjour Christophe — Secretariat est installé. Voulez-vous le configurer maintenant ? Environ 2 minutes."_

**First session (≤8 minutes):**

1. Identity: full name + display name.
2. Claude generates DID locally, confirms: _"Votre clé est créée. Elle ne quitte jamais cet ordinateur."_ (Trustworthiness)
3. Christophe pastes the invite URL from Rafa.
4. Claude: _"Connecté à Rafa Ballestiero. Il peut maintenant vous écrire, et vous pouvez lui répondre."_
5. First envelope rehearsal: _"Voulez-vous lui envoyer un premier message ? Par exemple, 'Salut Rafa, c'est en place de mon côté.'"_
6. Claude composes draft, shows body verbatim, asks confirmation.
7. Christophe confirms; Windows Hello dialog (with custom headline). Stamp lands.
8. Claude: _"Envoyé. Rafa le recevra à sa prochaine vérification."_

**First week (≤4 short sessions):**

| Day | Behavior                                                                    | BCT/PDP at work                                   |
| --- | --------------------------------------------------------------------------- | ------------------------------------------------- |
| 0   | Install + first envelope sent                                               | Rehearsal, Tunneling                              |
| 1   | First envelope from Rafa received and read; first reply drafted             | Habit formation (reinforcement), Real-world feel  |
| 3   | First multi-paragraph correspondence (case discussion, jurimétrie question) | Behavioral substitution (replacing email-context) |
| 7   | First "any new envelopes?" pull-on-conversation                             | Habit formation locks in                          |

**Locked-out behaviors (anti-compulsion):**

- No streak counter ("you've used Secretariat 7 days in a row!")
- No notification ("you have a new envelope")
- No engagement metric surfaced anywhere
- No leaderboard, no compare-to-other-Themia-members
- No nudge if absent for N days
- No "recommended messages" or AI-generated suggestions to-send

## Calm technology considerations

The substrate's anti-compulsion stance aligns naturally with calm technology principles, but two boundary cases warrant care:

- **The pull-on-conversation prompt** ("By the way, you have 2 unread envelopes") risks becoming a soft notification. Bound: surface only at conversation-start, never mid-conversation; never with count >3 (just say "you have unread envelopes"); never with sender names (forces a separate query). Keeps it informational, not attention-grabbing.
- **The Windows Hello dialog with headline-hash** is deliberately attention-grabbing — but that's the _one_ attention spike the substrate justifies. It's the ceremony moment; it should command attention. Make sure no other surface fights for the same attention budget.

## Recommendations

**Locked decisions (no further analysis needed):**

1. MCP-only Windows install — no Tauri, no daemon, no tray. ([[project-mcp-is-primary-interface]] + [[project-v03-install-floor-mcp-only]] already settled.)
2. Code-signed .msi with EquanimiTech / Themia certificate — non-negotiable. SmartScreen trust gate is the entire onboarding.
3. Windows Hello for stamp ceremony, with custom-prompt headline + hash prefix. Verify API capability before locking.
4. Pull-on-conversation, never push. No notifications, no streaks, no counts. ([[project-no-read-receipts]] + [[feedback-review-session-model]].)
5. Onboarding entirely inside Claude Desktop, via a Claude skill at `.claude/skills/secretariat-onboarding/`. Tunneled, principal stays in the conversation interface they already know.

**Open decisions (need a call before pitch lands):**

1. **Bidirectional contact-add for invite claim.** Marcelo audit identified this as L-severity; Christophe will hit the same wall if not fixed. Defer to existing onboarding-redesign pitch or fold into this one.
2. **PV correction path.** When Christophe stamps a PV with a mistake, what's the recovery flow? `rectificatif` envelope cross-referencing original is the conventional French legal answer; document in `assemblee_generale`'s CLAUDE.md.
3. **First-launch heartbeat surface.** If MCP didn't load (config write failed, binary missing), Claude has no idea Secretariat exists — meaning no way to detect the failure mode. Need: a heartbeat tool (or resource) that Claude probes on session start; if absent, Claude can suggest re-running installer.

**Next steps:**

- This analysis is the input to a follow-on pitch: _Secretariat-on-Windows MCP-only floor_ — separate from the Themia channels foundation pitch, but a hard dependency on it (Christophe's onboarding terminates in a fully-formed channel tree he can subscribe to).
- Roundtable this with the analytics-channels idea (`/idea` captured 2026-05-12) — both are about who-can-access-what; possibly related architecturally.
- The PV correction path and the bidirectional contact-add fix are work items, not pitches; capture as `/pain`.

## References

- BCT groupings primarily drawn on: Goals & Planning (1), Shaping Knowledge (4), Natural Consequences (5), Associations (7), Repetition & Substitution (8), Antecedents (12), Identity (13).
- PDP categories primarily drawn on: Primary Task (Reduction, Tunneling, Tailoring) and System Credibility (Trustworthiness, Surface credibility, Authority, Real-world feel, Verifiability).
- MoAs anchored: Beliefs about capabilities, Beliefs about consequences, Memory/attention/decision processes, Valued self-identity.
- Sequel to and updates: `docs/audits/2026-05-04-onboarding-ux.md` (Marcelo first-attempt).
- Related memories: [[feedback-marcelo-first-attempt]], [[project-mcp-is-primary-interface]], [[project-v03-install-floor-mcp-only]], [[project-no-read-receipts]], [[feedback-review-session-model]], [[project-assemblee-generale-channel]], [[project-profile-two-names]], [[feedback-show-drafts-before-signing]].
