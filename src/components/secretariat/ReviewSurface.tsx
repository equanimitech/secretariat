// Review surface — the principal's chosen-time ritual for acting on
// queued correspondence. See `docs/milestones/2026-05-04-tauri-front-door.md`
// slice 3 and `memory/feedback_review_session_model.md`.
//
// No notifications, no push. Just two collections (inbox + review queue)
// and an explicit "Sync now" affordance. Drafting happens in the
// principal's AI assistant; this surface is for review + stamp.

import { useCallback, useEffect, useState } from 'react'
import {
  commands,
  type EnvelopeListing,
  type IdentityState,
  type SyncReport,
} from '@/lib/bindings'

type Selection =
  | { kind: 'inbox'; envelope: EnvelopeListing }
  | { kind: 'queue'; envelope: EnvelopeListing }
  | null

type EnvelopeBody = {
  body: string
  from: string | null
  to: string | null
  was_encrypted: boolean
}

export function ReviewSurface() {
  const [identity, setIdentity] = useState<IdentityState | null>(null)
  const [inbox, setInbox] = useState<EnvelopeListing[]>([])
  const [queue, setQueue] = useState<EnvelopeListing[]>([])
  const [selection, setSelection] = useState<Selection>(null)
  const [reading, setReading] = useState<EnvelopeBody | null>(null)
  const [syncing, setSyncing] = useState(false)
  const [lastSync, setLastSync] = useState<SyncReport | null>(null)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setError(null)
    const ident = await commands.currentIdentity()
    if (ident.status === 'ok') setIdentity(ident.data)

    const i = await commands.listInbox()
    if (i.status === 'ok') setInbox(i.data)
    else setError(i.error)

    const q = await commands.listReviewQueue()
    if (q.status === 'ok') setQueue(q.data)
    else setError(q.error)
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const handleSync = useCallback(async () => {
    setSyncing(true)
    setError(null)
    try {
      const result = await commands.syncNow()
      if (result.status === 'ok') {
        setLastSync(result.data)
        await refresh()
      } else {
        setError(result.error)
      }
    } finally {
      setSyncing(false)
    }
  }, [refresh])

  const handleSelect = useCallback(async (sel: Selection) => {
    setSelection(sel)
    if (!sel) {
      setReading(null)
      return
    }
    const r = await commands.readEnvelope(sel.envelope.file_path)
    if (r.status === 'ok') {
      setReading(r.data)
    } else {
      setError(r.error)
      setReading(null)
    }
  }, [])

  if (!identity) {
    return (
      <div className="flex h-full items-center justify-center p-8">
        <p className="text-muted-foreground">
          No identity yet. Onboarding will land here in slice 4.
        </p>
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col gap-4 p-4">
      <header className="flex items-center justify-between border-b pb-3">
        <div className="text-sm">
          <span className="text-muted-foreground">You: </span>
          <code className="rounded bg-muted px-2 py-0.5 text-xs">
            {identity.did}
          </code>
        </div>
        <button
          type="button"
          onClick={handleSync}
          disabled={syncing}
          className="rounded-md border px-3 py-1.5 text-sm hover:bg-muted disabled:opacity-50"
        >
          {syncing ? 'Syncing…' : 'Sync now'}
        </button>
      </header>

      {error && (
        <div className="rounded-md border border-destructive bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}

      {lastSync && (
        <p className="text-xs text-muted-foreground">
          Last sync: {lastSync.sent_envelopes} sent;{' '}
          {lastSync.per_relay.reduce((n, r) => n + r.inbound_count, 0)}{' '}
          inbound;{' '}
          {lastSync.per_relay.reduce((n, r) => n + r.auto_added_contacts, 0)}{' '}
          new contact(s).
        </p>
      )}

      <div className="grid flex-1 gap-4 overflow-hidden md:grid-cols-2">
        <EnvelopeColumn
          title="Inbox"
          subtitle="Received envelopes"
          items={inbox}
          selectedPath={
            selection?.kind === 'inbox' ? selection.envelope.file_path : null
          }
          onSelect={env => handleSelect({ kind: 'inbox', envelope: env })}
          empty="Nothing yet. New arrivals show up here after Sync."
        />
        <EnvelopeColumn
          title="Review queue"
          subtitle="Drafts awaiting your stamp"
          items={queue}
          selectedPath={
            selection?.kind === 'queue' ? selection.envelope.file_path : null
          }
          onSelect={env => handleSelect({ kind: 'queue', envelope: env })}
          empty="No drafts. Your AI assistant composes them; they queue here for review."
        />
      </div>

      {selection && reading && (
        <EnvelopeReader
          envelope={selection.envelope}
          read={reading}
          onClose={() => handleSelect(null)}
        />
      )}
    </div>
  )
}

function EnvelopeColumn({
  title,
  subtitle,
  items,
  selectedPath,
  onSelect,
  empty,
}: {
  title: string
  subtitle: string
  items: EnvelopeListing[]
  selectedPath: string | null
  onSelect: (env: EnvelopeListing) => void
  empty: string
}) {
  return (
    <section className="flex min-h-0 flex-col rounded-md border">
      <header className="border-b px-3 py-2">
        <h2 className="text-sm font-semibold">{title}</h2>
        <p className="text-xs text-muted-foreground">{subtitle}</p>
      </header>
      <ol className="flex-1 overflow-auto">
        {items.length === 0 ? (
          <li className="p-4 text-sm text-muted-foreground">{empty}</li>
        ) : (
          items.map(env => (
            <li
              key={env.file_path}
              className={
                'cursor-pointer border-b px-3 py-2 text-sm hover:bg-muted ' +
                (selectedPath === env.file_path ? 'bg-muted' : '')
              }
              onClick={() => onSelect(env)}
            >
              <div className="flex items-center gap-2">
                {env.encrypted && <span title="Encrypted">🔒</span>}
                {env.stamped && <span title="Stamped">✓</span>}
                <code className="truncate text-xs">
                  {env.from ?? env.to ?? 'unknown'}
                </code>
              </div>
              <p className="truncate text-xs text-muted-foreground">
                {filenameFromPath(env.file_path)}
              </p>
            </li>
          ))
        )}
      </ol>
    </section>
  )
}

function EnvelopeReader({
  envelope,
  read,
  onClose,
}: {
  envelope: EnvelopeListing
  read: EnvelopeBody
  onClose: () => void
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-6">
      <div className="flex max-h-full w-full max-w-3xl flex-col gap-3 rounded-lg border bg-background p-5 shadow-lg">
        <header className="flex items-center justify-between">
          <div className="text-xs text-muted-foreground">
            {read.from && <span>from <code>{read.from}</code></span>}
            {read.to && <span> · to <code>{read.to}</code></span>}
            {read.was_encrypted && <span> · decrypted on this device</span>}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-md border px-2 py-1 text-xs hover:bg-muted"
          >
            Close
          </button>
        </header>
        <pre className="flex-1 overflow-auto whitespace-pre-wrap rounded-md bg-muted p-4 text-sm">
          {read.body}
        </pre>
        <footer className="flex justify-between text-xs text-muted-foreground">
          <code className="truncate">{envelope.file_path}</code>
          {!envelope.stamped && (
            <span>
              Unstamped — stamp from CLI/MCP for now (Touch ID gate).
            </span>
          )}
        </footer>
      </div>
    </div>
  )
}

function filenameFromPath(p: string): string {
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'))
  return i >= 0 ? p.slice(i + 1) : p
}
