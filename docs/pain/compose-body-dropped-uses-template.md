---
status: open
severity: high
created: 2026-05-04
updated: 2026-05-04
---

# Compose silently dropped the body, wrote template instead — first message blocked

Raw capture — 2026-05-04. Marcelo's first attempt to send a reply via natural language to his Claude.

## What dad's Claude said back to dad (verbatim)

> Before stamping, the tool requires me to show you the actual file contents first (anti-phishing check). Reading it now.
> Detected template error; flagged content mismatch issue
>
> Detected template error; flagged content mismatch issue
>
> Hold up — I can't stamp this as-is. The compose tool didn't actually write "what's up?" into the file; it wrote the envelope template boilerplate instead. Here's what's currently in the draft, verbatim:
>
>     # Secretariat envelope template
>     This file is yours to customize. Claude reads it to know how to compose
>     envelopes on your behalf. The structure follows attentional-granularity
>     (gross → subtle, with a deepening pathway).
>     When composing an envelope, follow this shape:
>     ## Headline (3–6 words)
>     **Lede:** one sentence — the thing the reader must remember.
>     ...
>
> Re-reading the compose tool's note now: "Draft written to outbox via the AG template. Edit the file to insert the body, then stamp it manually." So the v0 compose tool just stages the template — the actual body has to be written into the file separately before stamping.
>
> A few options:
>
> 1. You edit the file at ~/.secretariat/outbox/.../20260504T171229Z-q6dzqg.md to replace the template with what's up?, then I'll stamp it.
> 2. I can shape "what's up?" to fit the template's headline/lede/body structure ... and write that as a properly composed note.
> 3. Skip the template entirely — but I don't have a file-write tool for arbitrary paths in this session, so this would still need you to edit the file manually.

## Observed

- Dad asked his Claude to send "what's up?" to Rafa.
- Compose MCP tool ran, dropped the body argument, wrote the template scaffold instead.
- Anti-phishing show-body check correctly caught the mismatch (good — the safeguard worked).
- Dad's Claude could not auto-fix because it has no general file-write tool — the only paths to repair are: dad opens a text editor, OR dad's Claude tries to shape the message into the template structure (option 2).
- Net result: dad cannot send a one-line reply without opening a text editor or accepting Claude's reshape of his sentence.

## Where observed

- Dad's machine, sec 0.1.1, MCP wired into Claude Code.
- Repro: any natural-language compose request via MCP `compose` tool with a `body` param.
- Fix already shipped in 0.1.2 at the protocol level (`crates/mcp/src/server.rs` now passes `body` through to `ComposeRequest`), BUT dad has 0.1.1 and there is no auto-update path — fix doesn't reach him without re-running install.

## Questions

- How should the show-body anti-phishing check report this failure mode upstream — silent skip vs noisy "compose tool ignored your body, here's what was written" so future bugs surface immediately?
- Should compose return a content hash AND echo of the body it wrote, so the calling Claude can verify _before_ stamp-time that the body landed?
- What's the auto-update path that lets fixes like this actually reach existing principals (not just new installs)?

Don't fix yet.
