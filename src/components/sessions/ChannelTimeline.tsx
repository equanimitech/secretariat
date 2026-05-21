import { useCallback, useEffect, useState } from 'react'
import { Check, Lock, Hash, Terminal } from 'lucide-react'
import { toast } from 'sonner'
import { commands, type EnvelopePreview } from '@/lib/bindings'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { unreadStore } from '@/components/explorer/unreadState'
import { renderPreviewMarkdown } from '@/lib/markdown/preview-render'
import { usePreferences } from '@/services/preferences'
import { OPEN_MARKDOWN_EVENT, type OpenMarkdownRequest } from './SessionTabs'
import type { ChannelTab } from './types'

const ENVELOPE_OPENED_EVENT = 'secretariat:envelope-opened'

interface ChannelTimelineProps {
  tab: ChannelTab
}

export function ChannelTimeline({ tab }: ChannelTimelineProps) {
  const [envelopes, setEnvelopes] = useState<EnvelopePreview[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const { data: preferences } = usePreferences()

  const refresh = useCallback(async () => {
    setError(null)
    const res = await commands.readChannelEnvelopes(tab.channelPath, 100)
    if (res.status === 'ok') setEnvelopes(res.data)
    else setError(res.error)
  }, [tab.channelPath])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const openEnvelope = useCallback((env: EnvelopePreview) => {
    unreadStore.markOpened(env.file_path)
    window.dispatchEvent(new CustomEvent(ENVELOPE_OPENED_EVENT))
    const detail: OpenMarkdownRequest = { path: env.file_path, name: env.filename }
    window.dispatchEvent(
      new CustomEvent(OPEN_MARKDOWN_EVENT, { detail })
    )
  }, [])

  // Launch Claude is a *channel-level* action — it opens the cognition
  // substrate with cwd set to this channel's root, not the envelope's
  // file. Hence it lives in the channel header, not the envelope
  // toolbar. See AGENTS.md notes + Hard Rule #8 (channel-dir IS the
  // activation surface).
  const onLaunchClaude = useCallback(async () => {
    const res = await commands.launchClaudeAt(
      tab.channelPath,
      preferences?.assistant_terminal ?? null
    )
    if (res.status === 'error') {
      toast.error(`Launch Claude failed: ${res.error}`)
    }
  }, [tab.channelPath, preferences])

  return (
    <div className="flex h-full w-full flex-col">
      <div className="flex items-center justify-between border-b border-border px-4 py-2">
        <div className="flex items-center gap-2 text-xs">
          <Hash className="h-3.5 w-3.5 opacity-60" />
          <span className="font-medium text-foreground">
            {tab.org ? `${tab.org} / ` : ''}{tab.channelName}
          </span>
          <span className="font-mono text-[10px] text-muted-foreground">
            {tab.channelPath}
          </span>
        </div>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void onLaunchClaude()}
            title="Launch Claude in this channel"
          >
            <Terminal className="h-3.5 w-3.5" />
          </Button>
          <Button variant="ghost" size="sm" onClick={() => void refresh()}>
            Refresh
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3">
        {error && (
          <div className="mb-3 rounded border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}
        {envelopes === null && (
          <div className="py-10 text-center text-sm text-muted-foreground">
            Loading…
          </div>
        )}
        {envelopes !== null && envelopes.length === 0 && (
          <div className="py-10 text-center text-sm text-muted-foreground">
            No envelopes in this channel yet.
          </div>
        )}
        <ol className="flex flex-col gap-2">
          {envelopes?.map(env => (
            <EnvelopeCard
              key={env.file_path}
              env={env}
              onOpen={() => openEnvelope(env)}
            />
          ))}
        </ol>
      </div>
    </div>
  )
}

function EnvelopeCard({
  env,
  onOpen,
}: {
  env: EnvelopePreview
  onOpen: () => void
}) {
  const when = env.at ? formatWhen(env.at) : 'unknown time'
  // AG-shape preference: sender-declared `title` wins over body slice
  // for the headline; `lede` wins over body slice for the one-liner.
  // When neither is present we render the markdown preview from Rust
  // (first ~3 lines of the body).
  const title = env.title?.trim() ? env.title.trim() : null
  const lede = env.lede?.trim() ? env.lede.trim() : null
  return (
    <li>
      <button
        type="button"
        onClick={onOpen}
        className={cn(
          'group flex w-full flex-col gap-1 rounded-md border border-border bg-card px-3 py-2 text-left transition-colors',
          'hover:border-foreground/30 hover:bg-accent/40'
        )}
      >
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
            <StampBadge stamped={env.stamped} encrypted={env.encrypted} />
            <span className="font-mono">{shortDid(env.from)}</span>
            {env.source && (
              <span className="rounded bg-muted px-1.5 py-0.5 text-[10px]">
                {env.source}
              </span>
            )}
            <span>{when}</span>
          </div>
        </div>
        {title && (
          <div className="text-sm font-semibold text-foreground">{title}</div>
        )}
        <div className="line-clamp-3 text-sm text-foreground">
          {env.encrypted ? (
            <span className="italic text-muted-foreground">
              [sealed — open to decrypt]
            </span>
          ) : lede ? (
            <span className="text-foreground/90">{lede}</span>
          ) : env.preview ? (
            <div className="flex flex-col gap-0.5">
              {renderPreviewMarkdown(env.preview, { maxLines: 3 })}
            </div>
          ) : (
            <span className="italic text-muted-foreground">[empty]</span>
          )}
        </div>
      </button>
    </li>
  )
}

function StampBadge({
  stamped,
  encrypted,
}: {
  stamped: boolean
  encrypted: boolean
}) {
  if (stamped) {
    return (
      <span
        title="Stamped — principal-attested"
        className="inline-flex items-center gap-0.5 rounded-full bg-amber-100 px-1.5 py-0.5 text-[10px] font-medium text-amber-900 dark:bg-amber-950 dark:text-amber-200"
      >
        <Check className="h-3 w-3" />
        stamped
      </span>
    )
  }
  if (encrypted) {
    return (
      <span
        title="Sealed wire form"
        className="inline-flex items-center gap-0.5 rounded-full bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground"
      >
        <Lock className="h-3 w-3" />
        sealed
      </span>
    )
  }
  return (
    <span
      title="Signed but not stamped — informational, not principal-attested"
      className="inline-flex items-center gap-0.5 rounded-full border border-border bg-background px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground"
    >
      unstamped
    </span>
  )
}

function shortDid(did: string | null): string {
  if (!did) return '—'
  if (did.startsWith('did:key:')) {
    const tail = did.slice(8)
    return `did:key:${tail.slice(0, 8)}…`
  }
  return did.length > 40 ? did.slice(0, 40) + '…' : did
}

function formatWhen(rfc3339: string): string {
  const d = new Date(rfc3339)
  if (Number.isNaN(d.getTime())) return rfc3339
  const now = new Date()
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate()
  if (sameDay) {
    return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
  }
  return d.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  })
}
