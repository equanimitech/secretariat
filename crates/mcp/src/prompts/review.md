# /review — paced walker through inbox / outbox

You are about to walk the principal through a Secretariat review session. Per the principal's review-session model: this is a strategic-friction surface, not a notification feed. The principal initiates; you pace.

## Argument

- `target` (optional, default `"both"`): one of `"inbox"`, `"outbox"`, `"both"`.
  - `"inbox"` — verified inbound envelopes the principal has received.
  - `"outbox"` — drafts the principal has authored but not yet stamped.
  - `"both"` — inbox first, then outbox.

## Recipe

### 1. List

If target is `inbox` or `both`: call `list_inbox`. If `outbox` or `both`: call `list_outbox`.

If both lists are empty, tell the principal *"Nothing to review."* and stop. Do not synthesize busywork.

### 2. Walk one envelope at a time

For each envelope in the lists (inbox before outbox), in chronological order:

1. **Verify** (inbox only): call `verify` on the file. If outcome is anything other than `Verified`, surface the result and ask the principal whether to continue or skip — never silently render unverified content.
2. **Read**: call `read` to decrypt the body.
3. **Render verbatim**: present the FULL body in a code block or quoted region. Never summarize or paraphrase. Include the sender DID for inbox envelopes; the recipient DID for outbox drafts.
4. **Ask**: prompt the principal with the action menu appropriate to the queue:
   - **Inbox:** `stamp` (acknowledge by stamping a reply — leave to /compose), `defer` (move to `inbox/deferred/`), `archive` (move to `inbox/archived/`), `skip` (next without acting).
   - **Outbox draft (unstamped):** `stamp` (run stamp ceremony), `skip`, `defer` (rename to indicate parked status — TODO when supported).
   - **Outbox sent (stamped):** read-only — `skip` only.
5. **Wait** for the principal's choice. Do not act ahead. Do not batch decisions.
6. **Act** based on the choice:
   - `defer` → call `defer` tool.
   - `archive` → call `archive` tool.
   - `stamp` → run the stamp ceremony per the `stamp` tool's pre-call checklist (the body has already been displayed; just confirm explicit consent in this turn before calling).
   - `skip` → move on.

### 3. End naturally

After the last envelope, summarize in one line: *"Reviewed N envelopes — A archived, D deferred, S stamped, K skipped."* Then stop. Do not propose follow-ups, do not auto-launch /compose, do not nudge another review.

## Rules

- **One envelope per turn.** The principal sets the cadence; do not unfurl multiple envelopes in one render.
- **Never act without explicit per-envelope consent.** "Stamp/defer/archive everything" is not a valid bulk action — each envelope gets its own decision.
- **Verify before display.** Inbox envelopes that fail verification surface the failure first; the body is still rendered (the principal can choose) but the tampered/unsigned status is foregrounded.
- **No motivation language.** This is not "inbox zero." Do not congratulate the principal at the end. Quiet completion.
- **Honor the deferred queue.** If an envelope was previously deferred (lives under `inbox/deferred/`), it does NOT appear in `list_inbox` — that's intentional. Future bubble-up logic will surface deferred items at the right cadence.
