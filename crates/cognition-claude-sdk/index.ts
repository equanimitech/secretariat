/**
 * Cognition sidecar — wraps @anthropic-ai/claude-agent-sdk behind a
 * line-delimited JSON-RPC protocol over stdin/stdout.
 *
 * The Rust `ClaudeCodeSdkAdapter` (crates/core/src/infrastructure/cognition/claude_code_sdk.rs)
 * spawns this binary once at app start and multiplexes all tab sessions
 * through the same pipe. One sidecar process, N concurrent turns keyed
 * by `session_id`.
 *
 * Protocol — inbound (Rust → sidecar), one JSON object per stdin line:
 *   { "cmd": "send",     "session_id": "<caller-uuid>", "channel_dir": "<abs>", "message": "...", "is_first_turn": bool, "model"?: "..." }
 *   { "cmd": "cancel",   "session_id": "<caller-uuid>" }
 *   { "cmd": "shutdown" }
 *
 * Protocol — outbound (sidecar → Rust), one JSON object per stdout line:
 *   { "kind": "text_delta",        "session_id": "<caller-uuid>", "text": "..." }
 *   { "kind": "tool_call_start",   "session_id": "<caller-uuid>", "id": "...", "name": "...", "input": {...} }
 *   { "kind": "tool_call_result",  "session_id": "<caller-uuid>", "id": "...", "output": {...} }
 *   { "kind": "thinking",          "session_id": "<caller-uuid>", "text": "..." }
 *   { "kind": "warning",           "session_id": "<caller-uuid>", "message": "..." }
 *   { "kind": "done",              "session_id": "<caller-uuid>", "stop_reason": "..." }
 *   { "kind": "error",             "session_id": "<caller-uuid>", "message": "..." }
 *
 * Caller-supplied `session_id` is a stable handle for one logical
 * conversation; the SDK's internal session_id is mapped here. First
 * turn = no `resume` option; subsequent turns = `resume: sdkId` from the
 * cached mapping.
 */

import {
  query,
  AbortError,
  type Options,
  type SDKMessage,
} from '@anthropic-ai/claude-agent-sdk'

/**
 * Resolve the on-disk `claude` executable. After `bun build --compile`,
 * the SDK's bundled `cli.js` lives in Bun's virtual FS (`/$bunfs/root/`)
 * and isn't reachable as a real path. We override `pathToClaudeCodeExecutable`
 * to point at the user's installed standalone binary.
 *
 * Resolution order:
 *  1. SECRETARIAT_CLAUDE_PATH env (Tauri adapter can pin a specific version)
 *  2. `Bun.which("claude")` — PATH lookup
 *  3. Hardcoded common install locations
 */
function resolveClaudePath(): string | null {
  if (process.env.SECRETARIAT_CLAUDE_PATH) {
    return process.env.SECRETARIAT_CLAUDE_PATH
  }
  const fromPath = Bun.which('claude')
  if (fromPath) return fromPath
  const home = process.env.HOME ?? ''
  for (const candidate of [
    `${home}/.local/bin/claude`,
    `${home}/.claude/local/claude`,
    '/usr/local/bin/claude',
    '/opt/homebrew/bin/claude',
  ]) {
    try {
      if (Bun.file(candidate).size > 0) return candidate
    } catch {}
  }
  return null
}

const CLAUDE_PATH = resolveClaudePath()

/**
 * Resolve the bundled `sec-mcp` binary path so every SDK session has
 * Secretariat tools available (per [[project_mcp_is_primary_interface]]).
 * The Tauri adapter pins this via `SECRETARIAT_SEC_MCP_PATH` env at
 * sidecar spawn; fall back to PATH lookup if unset (e.g. running the
 * sidecar standalone for debugging).
 */
function resolveSecMcpPath(): string | null {
  if (process.env.SECRETARIAT_SEC_MCP_PATH) {
    return process.env.SECRETARIAT_SEC_MCP_PATH
  }
  return Bun.which('sec-mcp')
}

const SEC_MCP_PATH = resolveSecMcpPath()

interface SendCmd {
  cmd: 'send'
  session_id: string
  channel_dir: string
  message: string
  is_first_turn: boolean
  model?: string
}

interface CancelCmd {
  cmd: 'cancel'
  session_id: string
}

interface ShutdownCmd {
  cmd: 'shutdown'
}

type InboundCmd = SendCmd | CancelCmd | ShutdownCmd

interface SessionState {
  /** SDK-generated session id; learned on first turn, used for resume. */
  sdkSessionId: string | null
  /** AbortController for the current in-flight turn. */
  abort: AbortController | null
}

const sessions = new Map<string, SessionState>()

function emit(event: Record<string, unknown>) {
  process.stdout.write(JSON.stringify(event) + '\n')
}

function getOrCreateSession(callerSessionId: string): SessionState {
  let s = sessions.get(callerSessionId)
  if (!s) {
    s = { sdkSessionId: null, abort: null }
    sessions.set(callerSessionId, s)
  }
  return s
}

async function handleSend(c: SendCmd) {
  const state = getOrCreateSession(c.session_id)

  if (state.abort) {
    state.abort.abort()
    state.abort = null
  }

  const abort = new AbortController()
  state.abort = abort

  const options: Options = {
    cwd: c.channel_dir,
    abortController: abort,
    includePartialMessages: true,
    // Load user-level settings (~/.claude/) for global skills/plugins
    // AND project-level (channel-dir/.claude/) for the channel's own
    // CLAUDE.md, skills, agents. SDK default is isolation mode (no
    // settings) — we explicitly opt in.
    settingSources: ['user', 'project'],
  }
  if (CLAUDE_PATH) options.pathToClaudeCodeExecutable = CLAUDE_PATH
  if (c.model) options.model = c.model
  if (SEC_MCP_PATH) {
    options.mcpServers = {
      secretariat: {
        type: 'stdio',
        command: SEC_MCP_PATH,
      },
    }
  }
  if (!c.is_first_turn && state.sdkSessionId) {
    options.resume = state.sdkSessionId
  }

  try {
    const q = query({ prompt: c.message, options })
    for await (const msg of q) {
      forward(c.session_id, state, msg)
    }
    emit({
      kind: 'done',
      session_id: c.session_id,
      stop_reason: 'end_turn',
    })
  } catch (err) {
    if (err instanceof AbortError) {
      emit({
        kind: 'done',
        session_id: c.session_id,
        stop_reason: 'cancelled',
      })
    } else {
      emit({
        kind: 'error',
        session_id: c.session_id,
        message: err instanceof Error ? err.message : String(err),
      })
    }
  } finally {
    if (state.abort === abort) state.abort = null
  }
}

function forward(
  callerSessionId: string,
  state: SessionState,
  msg: SDKMessage
) {
  if (
    msg.type === 'system' &&
    (msg as { subtype?: string }).subtype === 'init'
  ) {
    const sysMsg = msg as { session_id: string }
    if (sysMsg.session_id) state.sdkSessionId = sysMsg.session_id
    return
  }

  if (msg.type === 'stream_event') {
    const ev = (msg as { event: unknown }).event as
      | {
          type?: string
          delta?: { type?: string; text?: string; thinking?: string }
        }
      | undefined
    if (!ev) return
    if (ev.type === 'content_block_delta') {
      const d = ev.delta
      if (d?.type === 'text_delta' && typeof d.text === 'string') {
        emit({ kind: 'text_delta', session_id: callerSessionId, text: d.text })
      } else if (
        d?.type === 'thinking_delta' &&
        typeof d.thinking === 'string'
      ) {
        emit({
          kind: 'thinking',
          session_id: callerSessionId,
          text: d.thinking,
        })
      }
    }
    return
  }

  if (msg.type === 'assistant') {
    const m = (msg as { message: { content: Array<Record<string, unknown>> } })
      .message
    if (Array.isArray(m?.content)) {
      for (const block of m.content) {
        if (block.type === 'tool_use') {
          emit({
            kind: 'tool_call_start',
            session_id: callerSessionId,
            id: String(block.id ?? ''),
            name: String(block.name ?? ''),
            input: block.input ?? {},
          })
        }
      }
    }
    return
  }

  if (msg.type === 'user') {
    const m = (msg as { message: { content: unknown } }).message
    if (m && typeof m === 'object' && 'content' in m) {
      const content = (m as { content: Array<Record<string, unknown>> }).content
      if (Array.isArray(content)) {
        for (const block of content) {
          if (block.type === 'tool_result') {
            emit({
              kind: 'tool_call_result',
              session_id: callerSessionId,
              id: String(block.tool_use_id ?? ''),
              output: block.content ?? null,
            })
          }
        }
      }
    }
    return
  }

  if (msg.type === 'result') {
    const r = msg as {
      is_error?: boolean
      result?: string
      errors?: string[]
      subtype?: string
    }
    if (r.is_error) {
      const errText = (r.errors ?? []).join('; ') || (r.subtype ?? 'unknown')
      emit({ kind: 'error', session_id: callerSessionId, message: errText })
    }
    return
  }
}

function handleCancel(c: CancelCmd) {
  const state = sessions.get(c.session_id)
  if (state?.abort) {
    state.abort.abort()
  }
}

async function main() {
  process.stdout.write(JSON.stringify({ kind: 'ready' }) + '\n')

  const decoder = new TextDecoder()
  let buffer = ''

  for await (const chunk of Bun.stdin.stream()) {
    buffer += decoder.decode(chunk, { stream: true })
    let nl: number
    while ((nl = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, nl).trim()
      buffer = buffer.slice(nl + 1)
      if (!line) continue
      let cmd: InboundCmd
      try {
        cmd = JSON.parse(line) as InboundCmd
      } catch (e) {
        emit({
          kind: 'error',
          session_id: '',
          message: `bad json: ${String(e)}`,
        })
        continue
      }
      if (cmd.cmd === 'send') {
        void handleSend(cmd)
      } else if (cmd.cmd === 'cancel') {
        handleCancel(cmd)
      } else if (cmd.cmd === 'shutdown') {
        for (const s of sessions.values()) s.abort?.abort()
        process.exit(0)
      }
    }
  }
}

void main()
