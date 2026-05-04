# Reply to specific parts of a message (quote-reply / annotation)

Raw capture — 2026-05-05.

- "Can we reply to specific parts of a message?"
- Two implementation shapes, both additive on the existing envelope wire format:
  - **Range-based** — `in_reply_to: { doc_hash: "...", range: { start: 120, end: 245 } }`. Tight, but couples replies to byte offsets in the original — fragile if the original gets edited (it can't be, stamps freeze it, but still). Concretely scoped.
  - **Quote-include** — `in_reply_to: "...", quoted: "the exact text I'm responding to"`. Self-contained — the reply carries the quoted excerpt verbatim. Recipient renders it inline above the reply body. Doesn't depend on offsets surviving any rendering pass.
- Lean toward **quote-include**. Stamps on both ends mean the quoted text is verifiable against the original (the recipient can compute the parent envelope's hash and check the substring exists), but the reply stands on its own.
- UI lift: in `<EnvelopeReader>`, capture text selection → "Reply to this" affordance → opens compose drawer pre-filled with `quoted` + the original's `to:` flipped to a reply.
- Adjacent angle: this is the seed of an **annotation** primitive. A "comment" on a received envelope (private, stays in your inbox) is structurally identical to a quote-reply that you don't send. Could be the same internal data model, two different egress modes (kept vs sent).
- Adjacent angle 2: composes with the **multi-granularity** idea — a quote-reply at headline-granularity says "responding to the lede"; at body-granularity quotes the specific paragraph; at deep-granularity might quote a footnote or attachment.
- Adjacent angle 3: it's also how **signed errata** could work in a published-channel feed — author posts correction quoting the exact passage being corrected, cryptographically linked.
- Questions:
  - Does the protocol need ANY field for this, or is "quote it in markdown like email's `>` does" enough? Email-style quoting works socially but doesn't let the reader's app render the quote as a verifiable cross-reference. The structured field unlocks UI affordances.
  - When the reader opens a quote-reply, does the app fetch the parent envelope (if not already in inbox) to verify the quote? Yes — and if parent isn't available, render with a "unverified quote" caveat.
  - Does the principal compose this via Claude ("reply to the part where dad says X") or via UI text selection? Probably both — the AI-native bet is that Claude can extract the right substring; UI selection is the manual fallback.
- Don't shape yet.
