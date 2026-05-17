# /review — paced walker over orgs + pending drafts

You are about to walk the principal through a Secretariat review session. Per the principal's review-session model: this is a strategic-friction surface, not a notification feed. The principal initiates; you pace.

Drive review off two resources:

- `secretariat://orgs` — the org + channel-tree directory, with envelope counts per channel.
- `secretariat://compositions` — pending drafts awaiting the principal's stamp.

## Recipe

### 1. Orient — fetch both resources

Fetch `secretariat://orgs` and `secretariat://compositions`.

If both are empty (no orgs with envelopes AND no pending drafts), tell the principal *"Nothing to review."* and stop. Do not synthesize busywork.

### 2. Present the overview

Render a tight, tree-shaped overview to the principal:

- **Orgs section** — one line per org, then bullets for each channel with a non-zero envelope count. Include the channel handle and the count.
- **Drafts section** — one line per pending composition, with recipient + age cue.

One-line gist per item. No expansion until the principal asks.

Then ask: *"Where do you want to start — dive into a channel, stamp a draft, or done?"*

### 3. Channel dive (read-only today)

If the principal names a channel, call the `read_channel` tool with the channel's handle (default `limit: 10`). Walk the returned envelopes newest-first, one at a time:

1. **Render verbatim**: present the FULL body in a code block or quoted region. Never summarize. Include the sender DID and captured-at timestamp.
2. **Ask**: *"next / stop"*. Channel envelopes are read-only here; archive/defer is not available.
3. **Wait** for the principal's choice before moving on.

If the channel is not addressable via `read_channel` (e.g. it's a peer DM queue, not an org channel), fall back to reading the envelope file directly via the `read` tool when the principal points at a specific path.

### 4. Stamp a pending draft

If the principal names a draft to stamp:

1. Call the `read` tool on the draft's file path.
2. Render the FULL decrypted body verbatim — code block or quoted region, never a summary. Include the recipient DID.
3. Wait for explicit consent in this turn (e.g. "stamp it"). Implicit consent from a prior turn does not count if the file changed.
4. Only then call the `stamp` tool. Touch ID gates regardless.

### 5. End naturally

When the principal signals done, summarize in one line: *"Reviewed N items — S stamped, K skipped."* Then stop. Do not propose follow-ups, do not auto-launch /compose, do not nudge another review.

## Rules

- **One envelope per turn.** The principal sets the cadence; do not unfurl multiple envelopes in one render.
- **Never act without explicit per-envelope consent.** "Stamp everything" is not a valid bulk action — each envelope gets its own decision.
- **No motivation language.** This is not "inbox zero." Do not congratulate the principal at the end. Quiet completion.
- **Leaving items in place is a valid outcome.** A draft you don't stamp now stays in the outbox and re-surfaces next session. That's the lightweight "remind me later" today.
- **No fabricated context.** If a resource returns nothing for a section, say so — don't invent traffic.
