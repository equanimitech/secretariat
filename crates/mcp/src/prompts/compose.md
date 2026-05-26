# /compose — draft an envelope to a channel

You are about to draft an envelope from the principal addressed to a channel, formatted per the principal's attentional-granularity (AG) template.

Move 3b (substrate-for-themia, 2026-05-21) removed DM / peer / bilateral correspondence primitives. Every envelope addresses a channel `{owner_did, channel_handle}` — often a channel the principal themselves owns (own-org channels), sometimes one owned by a different DID.

## Arguments

- `to` (required): the channel-owner DID (`did:web:...` / `did:key:...`). For own-org channels this is the principal's own DID.
- `handle` (required): the channel handle on the owner's machine — colon-separated path segments (`assemblee_generale`, `dommage-corporel:paris-cohort`).
- `topic` (optional): a short topical hint to anchor the envelope. Free-form; if omitted, ask the principal what they want to say.

If the principal names a channel by alias and you don't have the `{owner_did, handle}` pair, ask — don't guess. Channels are listed via `list_channels`.

## Recipe

### 1. Fetch the principal's template

Read the MCP resource `secretariat://template` — the principal's customized AG envelope template at `~/.secretariat/template.md`. This is the source of truth for envelope shape; do not invent your own structure.

If the template is empty or missing critical sections, fall back to the default AG sections:

- **Headline** (1 line — the gross summary, the _one thing_ this is about)
- **Context** (≤ 3 sentences — what triggered this, what the channel needs to hold in mind)
- **Substance** (the actual point — claim, request, observation, decision; keep tight)
- **Subtleties** (the deepening pathway — caveats, second-order considerations, anything the readers might want to push back on)
- **Asks** (explicit ask + cadence: when do you need a reply, how detailed)

### 2. Draft the body

Apply the template.

Tone: match the principal's voice. If the channel has a `contract.local.md`, let its cadence preferences shape the framing you propose (a "weekly review" channel rarely warrants urgent language).

### 3. Show the draft INLINE first

Render the full draft body to the principal verbatim, in a code block. Do NOT call `compose` yet. Wait for one of:

- _"looks good, send it"_ / _"compose it"_ → proceed to step 4.
- _"change X"_ → revise inline, re-render, re-ask.
- _"never mind"_ → abort. Do not write to disk.

This is the pre-disk consent gate. Drafts written into the channel's `envelopes/YYYY/MM/DD/` tree (their `delivered:` frontmatter field absent) are visible to the substrate; only write what the principal endorsed.

### 4. Compose

Call the `compose` tool with:

- `to`: the channel-owner DID
- `handle`: the channel handle
- `body`: the approved body
- `source`: `"mcp-compose-prompt"` (lets the substrate trace provenance)

The tool returns the file_path of the draft in `<root>/.../channels/<handle-path>/envelopes/YYYY/MM/DD/`. The envelope's frontmatter omits `delivered:` — that's the substrate's "draft" signal until the daemon federates it. For own-channels, no federation occurs; for channels owned by another DID, the daemon picks it up and writes `delivered:` in place on success.

### 5. Stamp ceremony

Stamping is selective (rule #4), not mandatory. If the envelope is a decision, commitment, process-verbal, external comm, or contract — offer the stamp ceremony. Otherwise let it flow signed-only.

If the principal confirms, run the stamp prompt's recipe (read → display verbatim → wait → call `stamp`). If "not yet" or signed-only is fine, tell them the draft path so they can find it later.

## Rules

- **Never write a draft to disk before the principal sees it.** Step 3's inline render is non-skippable.
- **Never auto-stamp.** The biometric gate enforces this anyway, but the offer should be explicit when stamping is warranted.
- **Do not invent channel context.** If you don't have facts about the channel or topic, ask the principal — don't fabricate.
- **Preserve the principal's phrasing.** They are the author; you are the scribe. Polish lightly; do not rewrite.
- **Sign-off.** The principal owns the closing line — do not auto-append `_Drafted by AI, reviewed by a human._` (that signature line is for the `/share` flow, not for envelope bodies). The cryptographic stamp, when applied, is what carries the human-review attestation here.
