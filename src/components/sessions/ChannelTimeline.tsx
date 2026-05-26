import { useCallback, useEffect, useState, useSyncExternalStore } from 'react'
import { Lock, Hash, Terminal, BadgeCheck } from 'lucide-react'
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
    if (res.status === 'ok') {
      setEnvelopes(res.data)
      // Seed the unread store so the *next* refresh that discovers
      // new envelopes can compare against this baseline. First sight
      // ≠ unread (matches Explorer's quiet-on-first-launch contract).
      unreadStore.recordSeen(res.data.map(e => e.file_path))
    } else setError(res.error)
  }, [tab.channelPath])

  useEffect(() => {
    // One-shot Tauri IPC fetch when channelPath changes; no external store.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void refresh()
  }, [refresh])

  const openEnvelope = useCallback((env: EnvelopePreview) => {
    unreadStore.markOpened(env.file_path)
    window.dispatchEvent(new CustomEvent(ENVELOPE_OPENED_EVENT))
    const detail: OpenMarkdownRequest = {
      path: env.file_path,
      name: env.filename,
    }
    window.dispatchEvent(new CustomEvent(OPEN_MARKDOWN_EVENT, { detail }))
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
            {tab.org ? `${tab.org} / ` : ''}
            {tab.channelName}
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
        <div className="mx-auto w-full max-w-3xl">
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
  const sender = env.from_name?.trim() || shortDid(env.from)
  const isUnread = useIsUnread(env.file_path)
  return (
    <li>
      <button
        type="button"
        onClick={onOpen}
        className={cn(
          'group flex w-full flex-col gap-1.5 rounded-md border px-4 py-3 text-left transition-colors',
          // Quiet stamped-vs-not cue: stamped envelopes carry a stronger
          // border + card surface; unstamped envelopes sit flatter on
          // the page. No badge, no shout — the hierarchy itself is the
          // signal. See AGENTS.md hard rule #4 (stamped = authoritative).
          env.stamped
            ? 'border-foreground/30 bg-card shadow-sm hover:border-foreground/50'
            : 'border-border/60 bg-background hover:border-border'
        )}
      >
        <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
          {isUnread && (
            <span
              aria-label="unread"
              title="Unread"
              className="h-2 w-2 shrink-0 rounded-full bg-sky-500 dark:bg-sky-400"
            />
          )}
          <span
            className={cn(
              'font-medium text-foreground',
              !env.from_name && 'font-mono text-muted-foreground'
            )}
          >
            {sender}
          </span>
          <span aria-hidden>·</span>
          <span>{when}</span>
          {env.stamped && (
            <BadgeCheck
              className="h-3.5 w-3.5 text-amber-600 dark:text-amber-400"
              aria-label="Stamped — principal-attested"
            />
          )}
          {env.encrypted && (
            <Lock
              className="h-3 w-3 text-muted-foreground"
              aria-label="Sealed wire form"
            />
          )}
        </div>
        {title && (
          <div className="text-sm font-semibold leading-tight text-foreground">
            {title}
          </div>
        )}
        <div className="line-clamp-3 text-sm text-foreground/90">
          {env.encrypted ? (
            <span className="italic text-muted-foreground">
              [sealed — open to decrypt]
            </span>
          ) : lede ? (
            <span>{lede}</span>
          ) : env.preview ? (
            <div className="flex flex-col gap-0.5">
              {renderPreviewMarkdown(env.preview, { maxLines: 3 })}
            </div>
          ) : (
            <span className="italic text-muted-foreground">[empty]</span>
          )}
        </div>
        {(env.tags.length > 0 || env.source) && (
          <div className="mt-0.5 flex flex-wrap items-center gap-1">
            {env.tags.map(tag => (
              <span
                key={tag}
                className="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium text-muted-foreground"
              >
                {tag}
              </span>
            ))}
            {env.source && (
              <span
                title="Source"
                className="rounded-full border border-dashed border-border px-2 py-0.5 text-[10px] text-muted-foreground"
              >
                {env.source}
              </span>
            )}
          </div>
        )}
      </button>
    </li>
  )
}

function useIsUnread(path: string): boolean {
  // Treat as unread iff we've recorded it on a prior walk (so it isn't
  // brand new — Explorer's first-sight-doesn't-count rule applies) and
  // the principal hasn't opened it yet.
  return useSyncExternalStore(
    cb => unreadStore.subscribe(cb),
    () => unreadStore.wasSeenPreviously(path) && !unreadStore.isOpened(path),
    () => false
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
    return d.toLocaleTimeString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
    })
  }
  return d.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  })
}
