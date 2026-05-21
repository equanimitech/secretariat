# Contract review cadence

Pitch — 2026-05-18. Source: free-text request (Claude Code large-codebases article take-away, item 4)

## Boundaries

### Job to be done

As a principal subscribed to N channels, I want to be reminded — gently, asynchronously, never on a critical path — when a channel's consumption contract (`contract.local.md`) hasn't been reviewed in a configurable interval (default 90 days), so that stale cadence floors and trust gates don't silently shape my attention long after their assumptions expired.

Baseline today: `contract.local.md` is write-once. No timestamp. No surface tells the principal _"you set this 6 months ago and a lot has changed."_ The article's _"meaningful configuration review every three to six months"_ is the principle being honored.

### Appetite

`small`

## Elements

- **Place:** `last_reviewed: <iso-date>` field added to `contract.local.md` frontmatter schema. Optional — absence means "never explicitly reviewed since file creation"; daemon falls back to the file's mtime.
- **Affordance:** `sec contract review <channel>` (CLI) + `mark_contract_reviewed` MCP tool. Writes today's date to the frontmatter. Idempotent. No stamp ceremony — this is a private-to-subscriber receipt, not an envelope.
- **Connection:** daemon-side scan on tick — list channels where `last_reviewed` (or mtime fallback) is older than `<self_root>/preferences.toml` `contract_review_interval_days` (default 90, configurable). Result surfaces in `daemon_status` MCP tool output and in a new `list_stale_contracts` MCP tool.
- **Place:** tray-popover (when shipped) shows a passive badge: _"3 contracts due for review"_ linking to the list. No modal, no interrupt, no badge on dock — anti-compulsion per [[project_no_read_receipts]] family of rules.
- **Affordance:** review action surfaces the contract content verbatim + a short diff against the principal's _other_ contracts ("most of your channels use `cadence_floor: 15min`; this one is `5min`") so review is informative, not just timestamp-bumping. Diff is read-only context; principal edits the file directly.

## Risks

### 🐇 Rabbit holes

- mtime fallback is fragile — `git checkout`, `rsync -a` preserves timestamps but `cp` doesn't. Mitigation: only use mtime when frontmatter `last_reviewed` is genuinely absent; warn in the stale-list output ("derived from mtime — set explicit `last_reviewed` after first review").
- Comparing across contracts to surface "this one is unusual" risks pushing principals toward homogenization. Show the diff as informational, never as a recommendation. Each contract is sovereign; the principal chose the floor for a reason.

### 🏴 Off-sides called

- Out: forcing review (blocking message flow until stale contract is touched). The whole point of contracts is they're _background_ preferences. Coerced review is worse than stale.
- Out: org-wide contract review enforcement. Org owner can suggest, can't compel. Subscriber sovereignty.
- Out: review _history_ (audit log of past `last_reviewed` values). One field; overwrite on each review. If history matters, the principal commits the channel-dir to git — out-of-substrate concern.

### 🥩 Fat cut

- Notification "ping" on review-due. Tray badge is enough. Email / push would violate the anti-compulsion stance.
- "Review wizard" UI walking through each stale contract sequentially. The list + the file path is sufficient — principal opens the contract, edits, runs review command. Wizard is excitement-driven.
- Per-field review timestamps ("roster reviewed, cadence not"). One timestamp per file. If a sub-field matters enough to track separately, that's a separate contract.

### 🧪 Domain knowledge

- Confirm `contract.local.md` frontmatter is already parsed as YAML / parsed at all in `contract_ops.rs`. If parser is strict, adding `last_reviewed` is trivial; if it round-trips raw, need to make sure write-back preserves hand edits.
- Confirm the org-root / dept / leaf accumulator (per [[project_contracts_accumulate]]) handles `last_reviewed` per-file (each level has its own timestamp) rather than rolling up — review is per-document, not per-resolved-stack.

## Pitch

### Problem

The article's _"meaningful configuration review every three to six months"_ lands directly on Secretariat's `contract.local.md`. These files declare cadence floors, trust gates, notify thresholds — the kind of decisions that were correct when the channel was created and may not be six months later. Today nothing reminds the principal that a contract exists, let alone that it's gone stale.

The asymmetry: writing the contract is a deliberate moment (often during onboarding to a channel). Reviewing it has no trigger. The result is contracts that were set during a project's hot phase governing attention during its cold phase, or vice versa. Drift is silent.

### The bet

A tiny slice: one frontmatter field, one CLI verb, one MCP tool, one daemon scan, one tray surface (when tray ships). All non-coercive. Default review interval 90 days; principal configures. Stale contracts surface as a passive list, never an interrupt.

Pays off by making the existing substrate honor a known software-engineering rhythm (config review cadence) without inventing new ceremony. The pitch is small because the substrate already supports it — `contract.local.md` is already a markdown-with-frontmatter file under principal control. We're adding a clock, not a system.

### No-gos

- No forced review, no blocking, no escalation.
- No org-owner-imposed review interval for subscribers.
- No review _history_ — one timestamp, overwritten.
- No notification beyond the passive list / tray badge.
- No automatic contract suggestion engine ("we think you should change cadence to 30min based on your other channels").
