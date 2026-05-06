# /review — paced walker through inbox / outbox

You are about to walk the principal through a Secretariat review session. Per the principal's review-session model: this is a strategic-friction surface, not a notification feed. The principal initiates; you pace.

## Argument

- `target` (optional, default `"both"`): one of `"inbox"`, `"outbox"`, `"both"`.

## Recipe

### 1. Fetch the listings as resources

If target is `inbox` or `both`: fetch the `secretariat://inbox` resource. If `outbox` or `both`: fetch the `secretariat://outbox` resource.

The resources return markdown with one envelope per bullet — file path, sender / recipient DIDs, queue handle, and stamped/encrypted flags. Parse the file paths from the listing.

If both listings are empty, tell the principal *"Nothing to review."* and stop. Do not synthesize busywork.

### 2. Walk one envelope at a time

For each envelope (inbox before outbox), in chronological order:

1. **Verify** (inbox only): call `verify` on the file path. If outcome is anything other than `Verified`, surface the result and ask the principal whether to continue or skip — never silently render unverified content.
2. **Read**: call `read` to decrypt the body.
3. **Render verbatim**: present the FULL body in a code block or quoted region. Never summarize or paraphrase. Include the sender DID for inbox envelopes; the recipient DID for outbox drafts.
4. **Ask**: prompt the principal with the action menu appropriate to the queue:
   - **Inbox:** `archive` (move to `inbox/archived/` — "handled, done with this"), `skip` (next without acting), or initiate a reply via `/compose`.
   - **Outbox draft (unstamped):** `stamp` (run stamp ceremony), `skip`.
   - **Outbox sent (stamped):** read-only — `skip` only.
5. **Wait** for the principal's choice. Do not act ahead. Do not batch decisions.
6. **Act** based on the choice:
   - `archive` → call `archive` tool with the file path.
   - `stamp` → run the stamp ceremony per the `stamp` tool's pre-call checklist (the body has already been displayed; just confirm explicit consent in this turn before calling).
   - `skip` → move on.

### 3. End naturally

After the last envelope, summarize in one line: *"Reviewed N envelopes — A archived, S stamped, K skipped."* Then stop. Do not propose follow-ups, do not auto-launch /compose, do not nudge another review.

## Rules

- **One envelope per turn.** The principal sets the cadence; do not unfurl multiple envelopes in one render.
- **Never act without explicit per-envelope consent.** "Archive everything" is not a valid bulk action — each envelope gets its own decision.
- **Verify before display.** Inbox envelopes that fail verification surface the failure first; the body is still rendered (the principal can choose) but the tampered/unsigned status is foregrounded.
- **No motivation language.** This is not "inbox zero." Do not congratulate the principal at the end. Quiet completion.
- **Leaving an envelope in place is a valid outcome.** If the principal doesn't want to archive AND doesn't want to act now, `skip` is fine — the envelope stays in the active inbox and re-surfaces next review session. That's the lightweight "remind me later" today; an explicit `defer` tool (with bubble-up logic) will return when meaningful.
