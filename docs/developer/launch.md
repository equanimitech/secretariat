# `sec launch` — interactive cognition in a channel-bound cwd

`sec launch <handle> [--org <alias>]` opens the principal's chosen
interactive cognition CLI (default: Claude Code) with `cwd` set to the
target channel's on-disk home. The channel-dir is authoritative: its
`.claude/` tree (skills, agents, commands, project memory) loads
automatically, so a parent operator can fan into any channel's context
with one verb.

This is the human side of the launch/dispatch/root_path slice
(`docs/pitches/2026-05-13-launch-dispatch-root-path.md`). The headless
`dispatch` counterpart and the `bind` writer ship separately.

## Resolution

```text
sec launch channel:dev:secretariat --org equanimi.tech
└─ channels_root      = ~/.secretariat/orgs/equanimi.tech/channels
└─ default channel-dir = <channels_root>/dev/secretariat
└─ root_path?          read from <default>/contract.local.md
   ├─ set    → cwd = <root_path>      (typically a git repo on disk)
   └─ unset  → cwd = <default>        (substrate-native dir)
```

The override field lives in the same `contract.local.md` frontmatter as
`ChannelContract`'s `cadence_floor_minutes` / `min_trust` but doesn't
participate in the accumulate merge — bindings are per-device,
per-principal, never inherited from ancestors and never sent on wire.

## Binding a channel to a host directory

Until `sec bind` ships (separate slice), set the binding by hand:

```bash
mkdir -p ~/.secretariat/orgs/themia.pro/channels/themia
cat > ~/.secretariat/orgs/themia.pro/channels/themia/contract.local.md <<'EOF'
---
$type: tech.equanimi.secretariat.channelContract
root_path: /Users/rafa/Developer/themia
---
EOF
```

`sec launch channel:themia --org themia.pro` now opens Claude Code at
`/Users/rafa/Developer/themia`, picking up that repo's `.claude/`
tree.

If the bound path is a git repo, add a fenced block to its
`.gitignore` so Secretariat artifacts don't leak into shared history:

```text
# === secretariat ===
contract.local.md
envelopes/
outbox/
_ciphertext/
# === /secretariat ===
```

## Choosing the cognition substrate

`launch_command` + `launch_args` + `launch_env` in
`~/.secretariat/preferences.toml` decide *what* gets launched. Default
ships as Claude Code (`claude`); LM Studio routes Claude Code's wire
protocol at a local OpenAI-compatible endpoint via env vars only —
no fork of the CLI needed.

### Claude Code (default)

```toml
[cognition]
launch_command = "claude"
```

### LM Studio (local OpenAI-compatible endpoint)

```toml
[cognition]
launch_command = "claude"
launch_args = ["--model", "openai/gpt-oss-20b"]

[cognition.launch_env]
ANTHROPIC_BASE_URL = "http://localhost:1234"
ANTHROPIC_AUTH_TOKEN = "lmstudio"
```

Caveats from the upstream LM Studio integration
(`https://lmstudio.ai/docs/integrations/claude-code`):

- LM Studio must be running as a server before `sec launch` —
  start it from the app or `lms server start --port 1234`.
- Use a model with ~25k+ context length. Claude Code is context-
  intensive; smaller windows degrade the experience fast.
- If LM Studio has "Require Authentication" enabled, generate an API
  token there and set `ANTHROPIC_AUTH_TOKEN` to that value instead of
  the placeholder `lmstudio`.

### Other substrates

Any CLI that accepts `cwd` works the same way — `launch_command =
"my-cognition-wrapper"` or an absolute path to a shell script that
sets up its own env before exec'ing the real tool. The application
layer doesn't know which substrate it's launching; only the planner
adapter does.

## Inspecting what would happen

`--print-plan` emits the resolved plan as JSON without launching:

```bash
$ sec launch channel:themia --org themia.pro --print-plan
{
  "args": ["--model", "openai/gpt-oss-20b"],
  "command": "claude",
  "cwd": "/Users/rafa/Developer/themia",
  "env": {
    "ANTHROPIC_BASE_URL": "http://localhost:1234",
    "ANTHROPIC_AUTH_TOKEN": "lmstudio"
  }
}
```

## Process semantics

On Unix, `sec launch` replaces its own process with the planned
command via the POSIX `execvp` syscall — the shell's job-control sees
Claude Code directly, no `sec` parent left lingering. On non-Unix
platforms `sec` spawns the command and waits.

## Errors

| Error | Meaning |
| --- | --- |
| `invalid handle ...` | Handle didn't parse — see `QueueHandle::parse` |
| `not a channel handle` | Use `channel:foo` / `channel:foo:bar`, not `inbox:` / `area:` |
| `channel ... does not exist` | No `.channelDef` at the resolved path — `sec channels create` first |
| `launch_command is empty in preferences` | `[cognition] launch_command = ""` — pick a real binary |
| `could not launch ...` | OS-level spawn failure — typically the binary isn't on `$PATH` |
