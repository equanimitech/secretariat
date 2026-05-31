---
migrated_from: equanimi.tech/project/secretariat/cognition/20260521T145239Z-ubhce4.md
---

# opencode as alternative cognition launcher

Future improvement. Park behind v0.3 channels + roster gate, v0.4 attention routing.

**Why it fits sovereignty rule #5 (cognition pluggable):**

Opencode (<https://opencode.ai>) is a viable second cognition substrate alongside Claude Code. Single-binary install vs Claude Code's subscription tether. Strengthens the principal's ability to swap the brain — substrate AND model AND provider, all without vendor lock.

**Capabilities checked (2026-05-21):**

* **Stdio MCP servers** — supported. Config in `opencode.json`:

  ```json
  {
    "mcp": {
      "secretariat": {
        "type": "local",
        "command": ["sec", "mcp", "serve"],
        "enabled": true
      }
    }
  }
  ```

  Same `sec-mcp` binary works unchanged.

* **75+ providers via AI SDK + Models.dev:**

  * Local: LM Studio, Ollama, llama.cpp

  * Routing: OpenRouter, Vercel AI Gateway, Cloudflare AI Gateway

  * BYOK: Anthropic, OpenAI, Bedrock, Vertex, Groq, DeepSeek, etc.

  * Credentials in `~/.local/share/opencode/auth.json` via `/connect`

**Scope when picked up:**

1. Add `opencode` preset to `[cognition] launch_command / launch_args` in `preferences.toml`.
2. `sec mcp install --target opencode` writes `opencode.json` (currently only writes Claude Code config).
3. Channel-dir activation parity — opencode walks `AGENTS.md`, not `.claude/`. Verify skill tree-walk equivalence; may need symlink or dual-write.
4. Stamp ceremony verification — confirm opencode shows full body verbatim before `stamp` tool-call (no auto-summarize). This is the load-bearing safety property.

**Not a wedge driver:** Marcelo and Christophe use Claude Code today. Ship when a concrete principal asks for local-only cognition or non-Anthropic models.
