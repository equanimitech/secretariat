# /stamp — explicit stamp ceremony

You are about to walk the principal through stamping a draft envelope. This prompt formalizes the multi-turn ceremony that the `stamp` tool's description requires — exposing it as an invocable verb so the principal can ask for it explicitly.

## Argument

- `file` (required): the absolute path to a draft envelope under `<root>/<alias-of-recipient>/channels/<handle-path>/envelopes/YYYY/MM/DD/`. Drafts are envelopes whose frontmatter lacks the `delivered:` field. If omitted, fetch the `secretariat://compositions` resource, render the pending drafts, and ask the principal to pick one by path.

## Recipe (non-skippable — phishing / habituation defense)

### 1. Read the draft

Call `read` on the file path. This decrypts the body if encrypted and returns the envelope's `to`, `from`, and full text.

### 2. Render the body verbatim

Display the FULL body of the envelope to the principal — code block or quoted region, never a summary, never a paraphrase. Include the recipient DID and (if present) the depth + urgency frontmatter.

This is the mandatory display gate. If you skip this step the principal cannot give informed consent and the ceremony is broken.

### 3. Wait for explicit consent

Ask: _"Stamp this envelope?"_ Wait for the principal's response in this turn.

Accept as consent: _"yes"_ / _"stamp it"_ / _"go ahead"_ / _"sign"_ / unambiguous affirmatives.

Reject as non-consent: silence, _"hold on"_, _"let me think"_, _"change X"_, anything ambiguous. If the principal asked for a change, treat the ceremony as aborted — the body needs to be rewritten in `/compose` first, then `/stamp` re-run. Implicit consent from a prior turn does NOT count if the body has changed since you displayed it.

### 4. Call `stamp`

Once consent is explicit, call the `stamp` tool with the file path. The tool:

1. Computes the canonical body hash.
2. Triggers the platform biometric gate (Touch ID on macOS).
3. The Touch ID dialog reason string carries the document's first-line headline + a short hash prefix.

**The principal should cross-check the dialog reason against the body you displayed in step 2.** If they differ, the principal should cancel — it means a different file was about to be stamped. If they match, authenticate.

The tool blocks until biometric confirmation or cancellation.

### 5. Surface the outcome

On success, render: signer DID, stamp timestamp, document hash, and stamped path. Tell the principal the envelope is now ready to be transmitted by the daemon at the next sync.

On Touch ID cancellation, surface the error verbatim. Do not retry without re-running the ceremony from step 1 (the body or file may have changed).

## Rules

- **Steps 1, 2, 3 are non-skippable.** Do not call `stamp` without having displayed the body verbatim and received explicit consent in the same turn.
- **Never paraphrase.** A summary is not a body. The principal is signing the bytes, not a vibes-summary.
- **One file per ceremony.** Do not bulk-stamp. Each envelope gets its own ceremony.
- **If the file is already stamped**, the tool will error unless `force: true`. Do not pass `force` without the principal's explicit instruction; re-stamping invalidates prior trust.
- **If the file path doesn't match the principal's expectation** — e.g. they asked to stamp envelope-A but you're holding path-to-envelope-B — abort and ask. File-path drift is a ceremony violation.
