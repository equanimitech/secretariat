# Onboarding UX Audit — Marcelo's first envelope

Date: 2026-05-04. Scope: every surface a non-developer principal touches
from "Rafa wants to send me an envelope" to "I've replied with a stamped
envelope of my own." Dad as proxy for Christophe and beyond.

## Field feedback (Marcelo, first attempt, 2026-05-04)

Direct quote from Rafa relaying dad: *"It was very clunky. He didn't know
if it was installed or not. Didn't really make that much sense for them."*

Specific signals:

- **No post-install confirmation.** The install script prints to stderr;
  dad scrolled past it and was unsure whether anything happened.
- **No "you're all set" moment.** After install + restart Claude + claim
  invite, there's no surface that says "you're done, ready to receive."
- **The whole flow doesn't cohere as a single story for the user.** It's
  three disconnected steps (Terminal install → Claude restart → Claude
  invite claim) without a visible thread tying them together.
- **Sending messages is a hassle.** Compose → manual edit → stamp → wait
  for hourly daemon tick. No "send now" affordance.
- **No two-way contact adding.** Invite claim is one-way today — claimer
  learns inviter, inviter doesn't learn claimer without manual DID exchange.
- **No way to easily check for new messages.** Missing `inbox_sync` MCP
  tool. Claude can only `list_inbox` what the hourly daemon has pulled.
- **`stamp` succeeds silently when daemon is down.** "Stamped but daemon
  isn't running so nothing is being sent yet" — file sits in outbox, no
  warning, principal thinks it's sent.
- **Daemon not auto-started at install time.** `install.sh` wires the MCP
  binary but doesn't run `sec daemon install` — user has to discover and
  ask Claude to install daemon as a separate step.

This trumps the abstract gaps below. v0.1.x is now distribution-shaped:
DMG + auto-update + daemon-on-install + state-visible-at-every-step.

## Distribution decision (locked 2026-05-04)

**Path B — Developer ID-signed + notarized DMG.** Apple Developer enrollment
already in place. Real install experience, no Gatekeeper warning. Auto-update
via Sparkle (or simpler: GitHub releases polling at first; Sparkle in v0.3).

## Surface walk

| #  | Step                | Today                                                     | Should be                                              | Sev |
|----|---------------------|-----------------------------------------------------------|--------------------------------------------------------|-----|
| 0  | Install feedback    | stderr scrolling, no clear "done" signal                  | Visible end-state: green check + next-step instruction | **L** |
| 1  | Receive install msg | Rafa hand-writes one-liner per recipient arch             | Stable URL, arch auto-detected                         | M   |
| 2  | Install             | `curl \| tar \| bash` in Terminal                         | One-click .pkg or `brew install equanimi/tap/sec`      | L   |
| 3  | Restart Claude      | Manual quit + reopen, no prompt                           | Installer shows "now restart Claude" + verifies        | S   |
| 4  | Receive invite URL  | Hand-pasted by Rafa via iMessage                          | Same (acceptable — it's a personal invitation)         | —   |
| 5  | Claim invite        | "Ask Claude to claim this URL"                            | Same — works via `invite_claim` MCP tool, but Claude needs to confirm "you're now connected to <inviter>" | S |
| 6  | Send DID to Rafa    | Manual copy-paste from Claude output                      | Auto-bidirectional contact (relay returns claimer DID) | L   |
| 7  | Wait for envelope   | Silence — no notification when one arrives                | macOS push or daemon → Claude prompt                   | L   |
| 8  | Check inbox         | `inbox_sync` MCP tool **MISSING** — only `list_inbox` exists | `inbox_sync` tool: poll relay, return new count    | L   |
| 9  | Read envelope       | `read` MCP tool exists ✅                                  | Same                                                   | —   |
| 10 | Compose reply       | `compose` MCP tool exists ✅                               | Same                                                   | —   |
| 11 | Stamp reply         | `stamp` MCP tool exists, Touch ID gates                   | Same — the stamp is the principal moment, keep it      | —   |
| 12 | Send reply          | Daemon ticks every 60min                                  | Push on stamp, or 1–5 min poll                         | L   |
| 13 | Receive updates     | Manual: re-run install                                    | `sec self-update`, daemon checks `/healthz` weekly     | M   |

Severity: **L** = blocker for next attempt. **M** = ship for v0.3. **S** = keep but document.

## Defaults that are wrong

- `poll_interval_minutes = 60` — should be 5 (or push). Hourly is the
  difference between "messaging" and "mailbox."
- `sec contact add --relay` required for did:key — should default to
  first registered relay (already implemented for `sec invite create`).
- `sec daemon serve` exposed at all — should be daemon-internal.
  Foreground serve was an MVP artifact; LaunchAgent is the real path.
- No `inbox_sync` MCP tool — every other primitive has one, this gap
  forces Terminal use.
- Install script ends with stderr noise instead of a green-check end-state.

## What "right" looks like for v0.2.0

A non-developer's complete experience, end to end:

1. iMessage from inviter:
   > "Install: <stable URL>. After install, paste this URL into Claude:
   > <invite URL>"
2. User clicks install URL → .pkg → installed → big visible "✓ Installed.
   Now open or restart Claude Code. You'll know it's wired when Claude
   says hi."
3. User opens Claude → pastes invite URL → Claude responds: "Connected to
   Rafa. You're ready. I'll let you know when you have envelopes."
4. macOS notification: "Rafa sent you an envelope."
5. User in Claude: "show it to me." Claude shows.
6. User: "reply with X." Claude composes, shows draft, asks: "stamp?"
7. User: yes. Touch ID. Sent. Notification: "delivered."

No Terminal. No DIDs pasted. No "what's a relay." The DID is real,
the cryptography is real, the stamp is real — but the principal sees
none of the plumbing.

## Work blocks (ordered by current priority)

0. **Install confirmation surface.** Replace stderr noise with a clear
   end-state: ASCII checkmark + DID generated + next step. Make
   `install.sh` end with a single boxed message that survives scrollback.
   ~30 lines.
1. **Inbox-sync MCP tool + push trigger on enqueue.** The hole that broke
   today's demo. ~80 lines.
2. **5-min poll default** + LaunchAgent reload after install. ~40 lines.
3. **Auto-bidirectional contact on invite claim.** Relay endpoint
   returns claimer DID to inviter on next poll → daemon adds to contact
   book. ~100 lines.
4. **`sec contact add` defaults `--relay` to first registered.** 10 lines.
5. **macOS notification adapter** so envelopes arriving surface to dad
   without him asking. Requires entitlement on signed bundle; for v0.2
   ship as terminal-bell + log line, defer push to v0.3. ~50 lines.
6. **`.pkg` installer + Homebrew tap.** ~half day.
7. **`sec self-update` + version-skew warning in daemon.** ~100 lines.

Estimated: 2–3 focused days for blocks 0–4, another 1–2 for 5–7.

## What this audit does not cover

- Reply path UX from dad → Rafa (mirror of send path; same fixes apply).
- Multi-recipient envelopes (out of scope for v0.2).
- Christophe-specific surfaces (legal brief intake) — separate audit when
  we get there.
- GUI / Tauri ceremony surface — explicitly future per AGENTS.md.

## Open questions (need decisions before block 5+)

- Push notifications: macOS-native (`UNUserNotificationCenter`) requires
  signed app bundle. Acceptable to ship unsigned for v0.2 with a "scary
  open" flow, or block on signing infra?
- Auto-update: in-place tarball replace works for `sec` binary; LaunchAgent
  may need re-load. Or do we go full Sparkle-style framework?
- `did:key` vs `did:web` default: today `did:key` is default and
  `did:web` is opt-in; principal-anchored DIDs (`did:web:<their domain>`)
  are stronger for the "cryptographic provenance" pitch but require a
  domain. Keep current default?
