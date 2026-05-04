# Native AI integration within the app

Raw capture — 2026-05-04.

- The Tauri app currently positions the principal's existing AI assistant (Claude Code, ChatGPT) as the drafting surface. The app is review-only.
- But: a "native" AI panel inside Secretariat would let the principal compose without leaving the app. They'd talk to Claude *inside* the review window, draft an envelope, see it land in the queue, stamp it.
- Architecturally compatible with the cognition-as-pluggable invariant — the in-app AI is just another `CognitionPort` adapter (BYOK Anthropic, local Ollama, Claude API, etc.).
- Tradeoff vs current model: the "your assistant drafts, then you open the app" model preserves the principal's existing AI surface (no learning curve, works with whatever they already use). A native panel is more vertical but locks more of the experience into Secretariat.
- Questions:
  - Is this v0.3 or v1? Ship-the-app-as-review-only first probably — see if the AI-elsewhere pattern actually works for Marcelo + Christophe.
  - Which substrate to ship first inside the app? Anthropic API (BYOK) is the lowest-friction.
  - Does the in-app composer use the same MCP tools the external AI uses, or does it talk to `secretariat-core` directly?
  - How does this interact with the review-session model? An in-app composer that prompts to-do every turn collapses the async ritual.
- Don't shape yet.
