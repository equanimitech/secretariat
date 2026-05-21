# Improve the copy in the Touch ID prompt

Raw capture — 2026-05-05.

Current Touch ID dialog copy (screenshot 2026-05-05):

> touchid-prompt is trying to Teste real
> [504e6342] — 20260504T220536Z-u2dw3q.md.
>
> Touch ID to allow this.

Three issues:

- **"touchid-prompt is trying to"** — the helper binary name leaks into the dialog. Should read as Secretariat speaking, not as a low-level utility named "touchid-prompt." The macOS API uses the calling binary's display name; we may need to name the helper differently or use a wrapper.
- **Grammar** — "trying to Teste real" parses as "trying to [headline]" which is broken. The headline is a noun, not a verb. Need a verb in the template: "_Stamp_ «Teste real»" or "_Sign_ «Teste real»".
- **Filename leaks** — `20260504T220536Z-u2dw3q.md` is the on-disk filename, irrelevant to the principal at the moment of authorizing a signature. Drop or move to a tiny secondary line.

What stays — per AGENTS.md rule 4, the hash prefix must remain. It's the cross-check between what Claude/the app displayed and what's actually being signed. Phishing defense.

Possible better template:

> Secretariat: sign "Teste real"
> hash 504e6342

Or, more conversational:

> Stamp "Teste real" to send it.
> document hash 504e6342

Adjacent angles:

- Localization — when the headline is in the recipient's language (Portuguese here), the surrounding scaffold text ("Stamp", "to send it") should match if possible. Or: keep the scaffold in the principal's UI language and only the headline in whatever language the envelope was drafted in.
- Truncation — long headlines (>40 chars) will wrap or get cut. Keep the hash visible regardless.
- Calmness — this dialog appears at the moment of intentional ritual. The copy should feel deliberate, not bureaucratic.

Questions:

- Do we need to rename the touchid-prompt helper to "Secretariat" so macOS shows the right name? Or is there an alternative API that lets us set the calling-app display name?
- Should the hash render as `504e6342` (raw hex) or formatted (`504e-6342` or `0x504e6342`) for legibility?
- For act variants (defer, vouch, dispute, redirect — `StampAct` enum), does the verb change? "Defer «Teste real»" reads differently from "Sign «Teste real»".

Don't shape yet.
