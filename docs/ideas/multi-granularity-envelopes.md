# Multi-granularity envelopes — generate versions at different scales

Raw capture — 2026-05-05.

- "Should the idea of attentive granularity basically generate versions at different scales of granularity?"
- Today's AG template is *one* envelope that progresses gross → subtle (Headline, Lede, Body, Go-deeper). The recipient reads top-to-bottom and stops where their attention runs out.
- The user's question flips the model: the scribe generates **multiple versions** of the same message at different attentional resolutions. The recipient's app picks the one that fits their current context — busy inbox-glance, deep review session, full-context deep-read.
- Possible shapes:
  - **Headline-only** (3–6 words) — surfaces in tray badge / inbox row preview
  - **Lede + Why-it-matters** (~3 sentences) — what shows when the recipient skims their inbox
  - **Full body** — what shows when the recipient opens the envelope
  - **Deep-context** — body + linked attachments + threading + provenance — what shows on long-read mode
- Each version is a stamped artifact in its own right (or one envelope with multiple body fields, all covered by the same stamp hash). Single stamp protects the whole bundle.
- AI-native angle: this is exactly what the scribe SHOULD generate. A human writes one body and stops. An AI assistant can produce all four resolutions simultaneously without much marginal effort. Pre-stamp, the principal reviews all of them; one stamp covers the bundle.
- Adjacent to *Hey.com's "Imbox vs The Feed"* — but more granular and per-message rather than per-channel.
- Adjacent to the *bubble-up* idea: when an envelope bubbles back, maybe it surfaces at a *different* granularity than the one that was glanced past last time.
- Questions:
  - Wire format: extend `$envelope:` frontmatter with `versions: { headline, lede, body, deep }`, or separate `$envelope.tech.equanimi.secretariat.envelope.v1` + a new `multi-granularity-envelope.v1` `$type` for forward-compat?
  - Does the recipient's app pick automatically (based on inbox depth, time of day, principal mood) or does the principal choose ("show me ledes, skip headlines")? Auto = magical but opaque; manual = explicit but loses the AG point.
  - Do replies inherit the granularity of the parent (reply at headline level → reply with headline)?
  - Does sending a single-version envelope (just body, no headline/lede) become a degraded form? Or does the scribe always generate all four, even for "quick replies"?
  - Cost: 4× the AI inference per envelope. Acceptable for the wedge audience? Might be cheap enough with caching (same context → same headline regardless of body length).
- Don't shape yet.
