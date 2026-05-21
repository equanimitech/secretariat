// Settings → Integrations. Shows whether the bundled `sec-mcp` is
// wired into Claude Code and Claude Desktop. The Tauri shell silent-
// wires both on app launch (per `wire_mcp_from_bundled_sec` in lib.rs,
// version-gated since 0.2.9), so this pane is mostly diagnostic — but
// when the bundled binary path doesn't match what the client has, the
// "Re-wire" button forces a re-run.

import { useCallback, useEffect, useState } from 'react'
import { CheckCircle2, AlertCircle, RefreshCw } from 'lucide-react'
import { commands } from '@/lib/bindings'
import type { IntegrationsStatus, IntegrationStatus } from '@/lib/bindings'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import { usePreferences, useSavePreferences } from '@/services/preferences'

type TerminalChoice =
  | 'terminal'
  | 'iterm'
  | 'ghostty'
  | 'wezterm'
  | 'alacritty'
  | 'claude-desktop'

const TERMINAL_OPTIONS: { value: TerminalChoice; label: string; hint: string }[] = [
  { value: 'terminal', label: 'Terminal.app', hint: 'macOS default' },
  { value: 'iterm', label: 'iTerm2', hint: 'iTerm.app' },
  { value: 'ghostty', label: 'Ghostty', hint: 'Ghostty.app' },
  { value: 'wezterm', label: 'WezTerm', hint: 'WezTerm.app' },
  { value: 'alacritty', label: 'Alacritty', hint: 'Alacritty.app' },
  { value: 'claude-desktop', label: 'Claude Desktop', hint: 'no terminal — opens Claude.app directly' },
]

export function IntegrationsPane() {
  const [status, setStatus] = useState<IntegrationsStatus | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [savedNote, setSavedNote] = useState<string | null>(null)

  const loadStatus = useCallback(async () => {
    const result = await commands.mcpIntegrationsStatus()
    if (result.status === 'ok') {
      setStatus(result.data)
    } else {
      setError(result.error)
    }
  }, [])

  useEffect(() => {
    void (async () => {
      const result = await commands.mcpIntegrationsStatus()
      if (result.status === 'ok') {
        setStatus(result.data)
      } else {
        setError(result.error)
      }
    })()
  }, [])

  const handleRewire = useCallback(async () => {
    setBusy(true)
    setError(null)
    setSavedNote(null)
    try {
      const result = await commands.rewireMcpIntegrations()
      if (result.status === 'error') {
        setError(result.error)
        return
      }
      setSavedNote('Re-wired.')
      setTimeout(() => setSavedNote(null), 2000)
      await loadStatus()
    } finally {
      setBusy(false)
    }
  }, [loadStatus])

  return (
    <div className="space-y-6 p-2">
      <AssistantLauncherSection />

      <section className="space-y-3">
        <div>
          <Label className="text-sm font-medium">MCP integrations</Label>
          <p className="text-xs text-muted-foreground">
            Where the Secretariat MCP server (`sec-mcp`) is currently
            wired. Claude Code and Claude Desktop pick this up at launch —
            once an integration shows ✓, you can use the Secretariat
            slash commands (<code className="rounded bg-muted px-1">/idea</code>,
            <code className="rounded bg-muted px-1">/review</code>,
            <code className="rounded bg-muted px-1">/compose</code>) in that client.
          </p>
        </div>

        {status ? (
          <div className="space-y-3">
            <IntegrationRow
              name="Claude Code"
              status={status.claude_code}
              bundledBinary={status.bundled_binary}
            />
            <IntegrationRow
              name="Claude Desktop"
              status={status.claude_desktop}
              bundledBinary={status.bundled_binary}
            />
          </div>
        ) : (
          <p className="text-xs italic text-muted-foreground">Checking…</p>
        )}

        <div className="flex items-center gap-2 pt-2">
          <button
            type="button"
            onClick={handleRewire}
            disabled={busy}
            className="inline-flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-sm hover:bg-muted disabled:opacity-50"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${busy ? 'animate-spin' : ''}`} />
            {busy ? 'Re-wiring…' : 'Re-wire integrations'}
          </button>
          {savedNote && (
            <span className="text-xs text-emerald-600 dark:text-emerald-400">
              {savedNote}
            </span>
          )}
        </div>
        {error && (
          <div className="rounded-md border border-destructive bg-destructive/10 p-2 text-sm text-destructive">
            {error}
          </div>
        )}

        {status?.bundled_binary && (
          <div className="border-t pt-4 text-xs text-muted-foreground">
            <p>
              Bundled binary:{' '}
              <code className="rounded bg-muted px-1 break-all">
                {status.bundled_binary}
              </code>
            </p>
          </div>
        )}
      </section>
    </div>
  )
}

function AssistantLauncherSection() {
  const { data: preferences, isLoading } = usePreferences()
  const savePreferences = useSavePreferences()

  const current: TerminalChoice =
    (preferences?.assistant_terminal as TerminalChoice | null | undefined) ?? 'terminal'
  const command = preferences?.assistant_command ?? ''

  const update = async (
    patch: Partial<{ assistant_terminal: string | null; assistant_command: string | null }>
  ) => {
    if (!preferences) return
    await savePreferences.mutateAsync({ ...preferences, ...patch })
  }

  return (
    <section className="space-y-3 border-b pb-6">
      <div>
        <Label className="text-sm font-medium">Assistant launcher</Label>
        <p className="text-xs text-muted-foreground">
          Where Secretariat opens your CLI assistant (Claude Code, Gemini, aider).
          Used by the home-screen launcher and <code>launch_assistant_in</code>.
        </p>
      </div>

      <div className="space-y-1.5 max-w-md">
        <Label htmlFor="terminal-select" className="text-xs">Terminal</Label>
        <select
          id="terminal-select"
          value={current}
          disabled={isLoading || savePreferences.isPending}
          onChange={e =>
            void update({
              assistant_terminal:
                e.target.value === 'terminal' ? null : e.target.value,
            })
          }
          className="w-full rounded-md border bg-background px-2 py-1.5 text-sm"
        >
          {TERMINAL_OPTIONS.map(opt => (
            <option key={opt.value} value={opt.value}>
              {opt.label} — {opt.hint}
            </option>
          ))}
        </select>
      </div>

      <div className="space-y-1.5 max-w-md">
        <Label htmlFor="assistant-command" className="text-xs">Command</Label>
        <Input
          id="assistant-command"
          type="text"
          placeholder="claude (default)"
          defaultValue={command}
          disabled={isLoading || savePreferences.isPending}
          onBlur={e => {
            const v = e.target.value.trim()
            if (v === (preferences?.assistant_command ?? '')) return
            void update({ assistant_command: v === '' ? null : v })
          }}
        />
        <p className="text-xs text-muted-foreground">
          Ignored when target is Claude Desktop.
        </p>
      </div>
    </section>
  )
}

function IntegrationRow({
  name,
  status,
  bundledBinary,
}: {
  name: string
  status: IntegrationStatus
  bundledBinary: string | null
}) {
  const pathMismatch =
    status.wired &&
    bundledBinary !== null &&
    status.binary_path !== null &&
    status.binary_path !== bundledBinary

  return (
    <div className="rounded-md border bg-muted/30 px-3 py-2.5">
      <div className="flex items-center gap-2">
        {status.wired && !pathMismatch ? (
          <CheckCircle2 className="h-4 w-4 shrink-0 text-emerald-600 dark:text-emerald-400" />
        ) : status.wired && pathMismatch ? (
          <AlertCircle className="h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400" />
        ) : (
          <AlertCircle className="h-4 w-4 shrink-0 text-muted-foreground" />
        )}
        <span className="text-sm font-medium">{name}</span>
        <span className="text-xs text-muted-foreground ml-auto">
          {!status.client_detected
            ? 'not installed'
            : !status.wired
              ? 'not wired'
              : pathMismatch
                ? 'stale wiring'
                : 'wired'}
        </span>
      </div>
      {status.wired && status.binary_path && (
        <code className="mt-1.5 block break-all text-xs text-muted-foreground">
          {status.binary_path}
        </code>
      )}
      {status.config_location && !status.wired && status.client_detected && (
        <p className="mt-1.5 text-xs text-muted-foreground">
          Config:{' '}
          <code className="rounded bg-background px-1 break-all">
            {status.config_location}
          </code>
        </p>
      )}
    </div>
  )
}
