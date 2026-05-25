# /compose — draft an envelope to a peer

You are about to draft an envelope from the principal to a peer, formatted per the principal's attentional-granularity (AG) template.

## Arguments

- `peer` (required): a contact's display-name slug (case-insensitive) or a DID (`did:web:...` / `did:key:...`). The compose tool resolves either.
- `topic` (optional): a short topical hint to anchor the envelope. Free-form; if omitted, ask the principal what they want to say.

## Recipe

### 1. Resolve peer

- If `peer` is a DID, use it directly.
- If `peer` is a slug, the compose tool will resolve it. If you want to pre-confirm, fetch the `secretariat://contacts` resource and find the match. If no match, tell the principal — they need to invite the peer first via `/onboard`.

### 2. Fetch the principal's template

Read the MCP resource `secretariat://template` — the principal's customized AG envelope template at `~/.secretariat/template.md`. This is the source of truth for envelope shape; do not invent your own structure.

If the template is empty or missing critical sections, fall back to the default AG sections:

- **Headline** (1 line — the gross summary, the _one thing_ this is about)
- **Context** (≤ 3 sentences — what triggered this, what the recipient needs to hold in mind)
- **Substance** (the actual point — claim, request, observation, decision; keep tight)
- **Subtleties** (the deepening pathway — caveats, second-order considerations, anything the recipient might want to push back on)
- **Asks** (explicit ask + cadence: when do you need a reply, how detailed)

### 3. Draft the body

Apply the template. Default depth: `subtle` (most envelopes). Default urgency: `whenever`. Override only if the topic genuinely warrants `gross` depth or `now`/`soon` urgency, and surface the choice to the principal.

Tone: match the principal's voice. If the recipient channel has a `contract.local.md`, let its cadence / depth preferences shape the default urgency you propose (a "weekly review" channel rarely warrants `now`).

### 4. Show the draft INLINE first

Render the full draft body to the principal verbatim, in a code block. Do NOT call `compose` yet. Wait for one of:

- _"looks good, send it"_ / _"compose it"_ → proceed to step 5.
- _"change X"_ → revise inline, re-render, re-ask.
- _"never mind"_ → abort. Do not write to disk.

This is the pre-disk consent gate. Drafts written into the queue's `envelopes/YYYY/MM/DD/` tree (their `delivered:` frontmatter field absent) are visible to the substrate; only write what the principal endorsed.

### 5. Compose

Call the `compose` tool with:

- `to`: the resolved DID or slug
- `body`: the approved body
- `depth`: chosen depth
- `urgency`: chosen urgency
- `source`: `"mcp-compose-prompt"` (lets the substrate trace provenance)
- `handle`: omit (defaults to `inbox`) unless the principal specified a non-default queue on the peer's machine.

The tool returns the file_path of the draft in `<root>/<alias-of-peer>/channels/<handle-path>/envelopes/YYYY/MM/DD/`. The envelope's frontmatter omits `delivered:` — that's the substrate's "draft" signal until the daemon federates it.

### 6. Stamp ceremony

Immediately offer to walk the stamp ceremony. The principal will say _"yes, stamp it"_ or _"not yet"_. If they confirm, run the stamp prompt's recipe (read → display verbatim → wait → call `stamp`). If "not yet," tell them the draft path so they can find it later.

## Rules

- **Never write a draft to disk before the principal sees it.** Step 5's inline render is non-skippable.
- **Never auto-stamp.** Step 7 always asks. The biometric gate enforces this anyway, but the offer should be explicit.
- **Do not invent peer context.** If you don't have facts about the peer or topic, ask the principal — don't fabricate.
- **Preserve the principal's phrasing.** They are the author; you are the scribe. Polish lightly; do not rewrite.
- **Sign-off.** The principal owns the closing line — do not auto-append `_Drafted by AI, reviewed by a human._` (that signature line is for the `/share` flow, not for stamped envelopes). The cryptographic stamp is what carries the human-review attestation here.
