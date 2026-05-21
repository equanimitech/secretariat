# Onboarding wizard — two steps, one ritual

Pitch — 2026-05-04. Source: free-text + memory pyramid (`feedback_review_session_model.md`, `project_invite_is_correspondence.md`, `project_settings_pane_shape.md`, `project_vision_tagline.md`, `equanimitech_principles.md`) + `docs/milestones/2026-05-04-tauri-front-door.md` slice 4.

**Hard dependency:** Tauri commands `set_profile`, `init_identity`, `claim_invite_url` (all shipped post-0.1.2). The wizard is wiring, not new plumbing.

## Boundaries

### Job to be done

When a non-developer principal opens `Secretariat.app` for the first time and is greeted by "no identity yet," I want them to walk through the smallest possible ritual that produces a working correspondence channel — a name on screen, a DID generated locally, Touch ID verified end-to-end, optionally connected to one peer — so that the next time they open the app they have something to do (review queue) instead of plumbing to configure.

Baseline today: principal sees the placeholder `<ReviewSurface>` empty state forever; `set_profile`/`init_identity`/`claim_invite_url` exist but no UI calls them.

### Appetite

`small` (a focused day). The plumbing is already in place from slices 1–3.

## Elements

Fat-marker sketch of the two screens.

### Screen 1 — _Set yourself up_

Single form. Single button.

- **Place:** centered modal pane, no chrome
- **Affordances:**
  - text field — _"What should we call you?"_ (placeholder: "Rafa", "Christophe", etc.)
  - small italic note — _"Your identity will be generated on this device. Touch ID will protect every signature."_
  - one button — _"Set me up"_
- **Connection lines:** click → triggers in this order:
  1. `set_profile(name)`
  2. `init_identity()` (generates ed25519 keypair + did:key)
  3. Touch ID test signature (no-op message, just to verify the gate works on this Mac)
  4. Best-effort `sec mcp install` (silent — wires Claude Code/Desktop if present)
  5. Avatar derived from DID hash (no upload yet — deterministic color block + initials from name)
- on success → screen 2; on Touch ID failure → "We need Touch ID to protect your signatures. [Try again] or [Skip — install Touch ID first]"

### Screen 2 — _Connect_

Three doors. None forced.

- **Place:** same modal, replaces screen 1 inline
- **Affordances:**
  - text field — _"Paste an invite URL"_ (auto-fills from clipboard if it contains `secretariat://` or `<relay>/v0/invite/`)
  - button — _"Claim it"_ (calls `claim_invite_url`)
  - secondary button — _"I'll invite someone"_ (calls `create_invite`, copies URL to clipboard, principal pastes elsewhere themselves)
  - tertiary button (link-styled) — _"Skip for now"_
- **Connection lines:**
  - claim → success → close wizard, land on review surface with "connected to <inviter>" line
  - invite → success → close wizard, "your invite URL is on the clipboard" hint visible briefly
  - skip → close wizard, land on empty review surface

### What's not a step

- **No welcome/explainer screen.** The italic note in screen 1 carries the model. A separate "what is Secretariat" screen earns its scope only if confused testers ask.
- **No success celebration.** Per equanimitech red lines (no completion animations, no streaks). The wizard ends; the app is open. That's the success state.
- **No avatar upload.** Deterministic color+initials default ships now; upload from Settings → Profile lands later.
- **No MCP toggle.** Wired silently. Settings → Assistant connections shows status; the wizard doesn't ask permission for a thing that's safe by default.

## Risks

### 🐇 Rabbit holes

- **Touch ID test signature** — what message do we sign? Not an envelope (no recipient yet). Possibly a fixed test domain `secretariat-touchid-test:v0:<DID>` so the signature is verifiable but useless as a real attestation. Decision needed — 1h spike.
- **Clipboard auto-detect** — Tauri's clipboard plugin reads on demand; doing so silently on screen 2 mount feels surveillance-y. Either ask consent first ("Read clipboard? — your invite URL might be there") or skip auto-detection and require explicit paste. Lean toward explicit paste.
- **Wizard re-entry** — what if the user closes the app mid-wizard? Current state-load (`getProfile() == null || currentIdentity() == null`) routes them back in. Need to handle "name set but identity failed" partial state cleanly — `init_identity` is already idempotent so this works.
- **Avatar color from DID hash** — pick a hashing/coloring strategy that's deterministic and visually distinct. Likely `hash(did) → HSL hue`, fixed S+L for legibility. Half-hour spike.

### 🏴 Off-sides called

- **Multi-step "tour"** of inbox / review queue / stamp ceremony. Tempting but not the JBTD. The wizard's job is to produce a working channel; product education happens in-product when the principal hits each surface for the first time (empty-state copy already does this in `<EnvelopeColumn>`).
- **Theme picker.** Hidden per `project_settings_pane_shape.md`.
- **Preferred language.** Same — i18n exists but only English ships.
- **Backup-key prompt.** "Save your recovery phrase!" UX is real but not v0.2. The DID can be regenerated; envelopes are stored locally; relay state is rebuildable. v1 problem.

### 🥩 Fat cut

- The "I'll invite someone" button on screen 2. Not all first-time users have a peer to claim from, but the inviter path is also: open command palette, run "Create invite," paste URL. The wizard doesn't have to be the only place this lives. **Decision:** keep the button; cost is 5 lines of TS, removes a "now what?" dead-end after onboarding.
- Animated transition between screens 1 → 2. A simple swap is fine; equanimitech's "boring by design" applies.

### 🧪 Domain knowledge

- **Touch ID UX on a brand-new Mac with no fingerprints enrolled** — the macOS dialog prompts the user to "Use Password" instead. Test this path before assuming Touch ID is universally available.
- **Mac without a Secure Enclave** (Intel, no T2 chip) — does the biometric gate degrade gracefully or hard-fail? Already handled in `crates/core/src/infrastructure/biometric.rs` (the `pick_gate` function), but worth confirming.
- **First-launch deep link** — if the principal arrives via `secretariat://invite/...` deep link, can we pre-populate the invite field on screen 2 instead of forcing a redundant paste? Probably yes — the deep link arrives via `onOpenUrl` which is already wired. Sequence the flows so wizard takes priority but the URL is queued for screen 2.

## Pitch

### Problem

Marcelo's first onboarding broke (`docs/pain/`, `memory/feedback_marcelo_first_attempt.md`) because installation was a four-step Terminal ritual followed by an MCP-driven Claude conversation followed by a manual DID exchange. The Tauri pivot replaced installation with a drag-to-Applications motion, but post-install the principal still hits a placeholder review surface with no path to a working channel.

The audit (`docs/audits/2026-05-04-onboarding-ux.md`) flagged "no 'you're all set' moment" and "the whole flow doesn't cohere as a single story." The wizard is the cohering story — but it must not become its own friction layer. The principal didn't open Secretariat to fill out a 6-step form; they opened it to send a stamped envelope to someone specific. The wizard pays for its own existence by producing a working channel in under 30 seconds.

### The bet

Two screens. Five seconds of typing. One Touch ID prompt. Outcome: the principal has a name on screen, a DID generated locally, a verified biometric gate, MCP wired into Claude (silently), and either a connected peer or a clean "ready to receive" state.

The bet pays off if a non-developer principal — Marcelo, Christophe, anyone in the wedge audience — can complete onboarding without phoning a friend. It's the difference between "Secretariat is something Rafa built" and "Secretariat is something I have."

### No-gos

- No welcome/explainer screen. The italic note + the in-product empty states do the explaining.
- No "save recovery phrase" UX. v1 problem.
- No celebration animation, completion percentage, streak, or "1/2 complete" progress bar. Equanimitech red lines (`equanimitech_principles.md`).
- No theme/language/keyboard-shortcut choices. Hidden per `project_settings_pane_shape.md`.
- No avatar upload. Deterministic-from-DID default ships; upload defers to Settings.
- No MCP-wire toggle. Silent default-on; status visible in Settings.
- No clipboard auto-read on screen 2. Explicit paste only.
