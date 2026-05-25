// Verb-first home surface: two modes, ambient blob status, no numbers.
//
// Ceremony split:
//   - **Review** = passive intake (peer inbox + local-capture queues).
//     Triage-shaped; nothing here is signable.
//   - **Sign** = active outbound. Outbox drafts grouped by recipient.
//     Stamp-shaped; the cryptographic ceremony happens here.
//
// Each mode renders its buckets as soft circles whose radius encodes
// log(count). Hover reveals the exact number; click spawns the
// principal's chosen assistant in their preferred terminal so the MCP
// tools take over from there. No clipboard prompt, no in-app composer —
// per `memory/project_mcp_is_primary_interface.md`.

import { useCallback, useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import {
  commands,
  type AppPreferences,
  type EnvelopeListing,
  type IdentityState,
  type Profile,
} from '@/lib/bindings'

type Mode = 'review' | 'sign'

interface Bucket {
  /// Stable key for React + tooltip ids.
  id: string
  /// Human-visible name. Capitalized queue slug for Review, contact
  /// display name (or DID prefix) for Sign.
  label: string
  /// Number of items in this bucket. Hover-only — never rendered as a
  /// digit on the resting screen.
  count: number
}

export function ReviewSurface() {
  const [identity, setIdentity] = useState<IdentityState | null>(null)
  const [profile, setProfile] = useState<Profile | null>(null)
  const [inboxItems, setInboxItems] = useState<EnvelopeListing[]>([])
  const [queueItems, setQueueItems] = useState<EnvelopeListing[]>([])
  const [preferences, setPreferences] = useState<AppPreferences | null>(null)
  const [mode, setMode] = useState<Mode>('review')
  const [syncing, setSyncing] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setError(null)
    const [ident, prof, inbox, queue, prefs] = await Promise.all([
      commands.currentIdentity(),
      commands.getProfile(),
      commands.listInbox(),
      commands.listReviewQueue(),
      commands.loadPreferences(),
    ])
    if (ident.status === 'ok') setIdentity(ident.data)
    if (prof.status === 'ok') setProfile(prof.data)
    if (inbox.status === 'ok') setInboxItems(inbox.data)
    else setError(inbox.error)
    if (queue.status === 'ok') setQueueItems(queue.data)
    else setError(queue.error)
    if (prefs.status === 'ok') setPreferences(prefs.data)
  }, [])

  useEffect(() => {
    // One-shot Tauri IPC fetch on mount; no external store to subscribe to.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void refresh()
  }, [refresh])

  // Keyboard: r → Review, s → Sign. Skip when focus is in an input
  // field so settings + onboarding stay typeable.
  useEffect(() => {
    function onKey(ev: KeyboardEvent) {
      const tag = (ev.target as HTMLElement | null)?.tagName ?? ''
      if (tag === 'INPUT' || tag === 'TEXTAREA') return
      if (ev.metaKey || ev.ctrlKey || ev.altKey) return
      if (ev.key === 'r') setMode('review')
      else if (ev.key === 's') setMode('sign')
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  const handleSync = useCallback(async () => {
    setSyncing(true)
    setError(null)
    try {
      const result = await commands.syncNow()
      if (result.status === 'error') setError(result.error)
      await refresh()
    } finally {
      setSyncing(false)
    }
  }, [refresh])

  const handleLaunch = useCallback(async () => {
    const result = await commands.launchAssistant(
      preferences?.assistant_terminal ?? null,
      preferences?.assistant_command ?? null
    )
    if (result.status === 'error') {
      toast.error(`Launch failed: ${result.error}`)
    }
  }, [preferences])

  const reviewBuckets = useMemo<Bucket[]>(() => {
    if (!identity) return []
    const selfDid = identity.did
    // Peer inbox — every received envelope counts as one bucket.
    const inboxBucket: Bucket = {
      id: 'peer-inbox',
      label: 'Inbox',
      count: inboxItems.length,
    }
    // Local captures — group by queue handle. Anything addressed to
    // self with a populated queue handle.
    const captureBuckets = new Map<string, Bucket>()
    for (const item of queueItems) {
      if (!item.to || !item.queue) continue
      if (item.to !== selfDid) continue
      const slug = sluggedQueue(item.queue)
      const id = `q:${slug.key}`
      const existing = captureBuckets.get(id)
      if (existing) existing.count += 1
      else captureBuckets.set(id, { id, label: slug.label, count: 1 })
    }
    return [inboxBucket, ...captureBuckets.values()]
  }, [identity, inboxItems, queueItems])

  const signBuckets = useMemo<Bucket[]>(() => {
    if (!identity) return []
    const selfDid = identity.did
    const peers = new Map<string, Bucket>()
    for (const item of queueItems) {
      if (!item.to || item.to === selfDid) continue
      if (item.stamped) continue
      const id = `peer:${item.to}`
      // Contact display-name lookup removed in Move 3b — peer labels are
      // DID-prefix only. Future: resolve via channel-roster cache.
      const label = truncateDid(item.to)
      const existing = peers.get(id)
      if (existing) existing.count += 1
      else peers.set(id, { id, label, count: 1 })
    }
    return [...peers.values()]
  }, [identity, queueItems])

  if (!identity) return null
  const displayName = profile?.display_name ?? null
  const buckets = mode === 'review' ? reviewBuckets : signBuckets
  const emptyHint =
    mode === 'review' ? 'Nothing to review.' : 'No drafts to sign.'

  return (
    <div className="flex h-full flex-col items-center justify-center gap-10 bg-background p-8">
      <header className="flex flex-col items-center gap-3 text-center">
        <PrincipalAvatar did={identity.did} name={displayName} />
        <p className="text-lg font-medium">{displayName ?? 'You'}</p>
      </header>

      <ModeToggle mode={mode} onChange={setMode} />

      <div className="flex min-h-[8rem] flex-col items-center gap-6">
        {buckets.length === 0 ? (
          <p className="text-sm text-muted-foreground">{emptyHint}</p>
        ) : (
          <>
            <BlobRow buckets={buckets} onClick={handleLaunch} />
            <Legend buckets={buckets} />
          </>
        )}
      </div>

      <button
        type="button"
        onClick={handleSync}
        disabled={syncing}
        className="text-xs text-muted-foreground underline-offset-4 hover:underline disabled:opacity-50"
      >
        {syncing ? 'Syncing…' : 'Sync now'}
      </button>

      {error && (
        <div className="rounded-md border border-destructive bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Mode toggle — Review / Sign pill, also bound to r/s keys.
// ---------------------------------------------------------------------------

function ModeToggle({
  mode,
  onChange,
}: {
  mode: Mode
  onChange: (m: Mode) => void
}) {
  return (
    <div
      role="tablist"
      aria-label="Home mode"
      className="inline-flex rounded-full border bg-muted p-1 text-sm"
    >
      <ModePill
        active={mode === 'review'}
        label="Review"
        hint="r"
        onClick={() => onChange('review')}
      />
      <ModePill
        active={mode === 'sign'}
        label="Sign"
        hint="s"
        onClick={() => onChange('sign')}
      />
    </div>
  )
}

function ModePill({
  active,
  label,
  hint,
  onClick,
}: {
  active: boolean
  label: string
  hint: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={onClick}
      className={
        'rounded-full px-4 py-1.5 transition ' +
        (active
          ? 'bg-background text-foreground shadow-sm'
          : 'text-muted-foreground hover:text-foreground')
      }
    >
      {label}
      <span className="ml-2 text-xs opacity-60">{hint}</span>
    </button>
  )
}

// ---------------------------------------------------------------------------
// Blob row + legend.
// ---------------------------------------------------------------------------

/// Pixel diameter of a blob, scaled by `log1p(count)`. Empty buckets
/// keep a small base disc so the legend remains readable. Clamped to a
/// sane upper bound so a noisy queue doesn't dominate the screen.
function blobDiameter(count: number): number {
  const base = 22
  const max = 96
  const scaled = base + Math.log1p(count) * 26
  return Math.min(max, Math.round(scaled))
}

function BlobRow({
  buckets,
  onClick,
}: {
  buckets: Bucket[]
  onClick: () => void
}) {
  return (
    <div className="flex flex-wrap items-end justify-center gap-6">
      {buckets.map(b => (
        <Blob key={b.id} bucket={b} onClick={onClick} />
      ))}
    </div>
  )
}

function Blob({ bucket, onClick }: { bucket: Bucket; onClick: () => void }) {
  const d = blobDiameter(bucket.count)
  const hasItems = bucket.count > 0
  const colorClass = hasItems
    ? 'bg-amber-400/80 hover:bg-amber-500/90 dark:bg-amber-300/70'
    : 'bg-emerald-400/40 hover:bg-emerald-500/60 dark:bg-emerald-300/40'
  return (
    <button
      type="button"
      onClick={onClick}
      title={`${bucket.label}: ${bucket.count}`}
      aria-label={`${bucket.label}, ${bucket.count} item${bucket.count === 1 ? '' : 's'}`}
      style={{ width: d, height: d }}
      className={
        'rounded-full transition-transform hover:scale-105 focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 ' +
        colorClass
      }
    />
  )
}

function Legend({ buckets }: { buckets: Bucket[] }) {
  return (
    <div className="flex flex-wrap items-center justify-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
      {buckets.map(b => (
        <span key={b.id}>{b.label}</span>
      ))}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Helpers — queue label + DID truncation.
// ---------------------------------------------------------------------------

interface SluggedQueue {
  /// Stable key for de-dup (the slug part).
  key: string
  /// Capitalized human-visible label.
  label: string
}

/// `inbox:triage` → `{ key: "triage", label: "Triage" }`. Drops the
/// namespace because the Review surface already implies "local intake."
/// Falls back gracefully on malformed handles.
function sluggedQueue(handle: string): SluggedQueue {
  const idx = handle.indexOf(':')
  const slug = idx >= 0 ? handle.slice(idx + 1) : handle
  const first = slug.charAt(0)
  const label = first === '' ? handle : first.toUpperCase() + slug.slice(1)
  return { key: slug, label }
}

function truncateDid(did: string): string {
  // `did:key:z6Mkb…XYZ` — keep enough to disambiguate without taking
  // the whole legend row.
  if (did.length <= 18) return did
  return `${did.slice(0, 12)}…${did.slice(-4)}`
}

// ---------------------------------------------------------------------------
// Avatar (unchanged shape; consolidated for the new layout).
// ---------------------------------------------------------------------------

function PrincipalAvatar({ did, name }: { did: string; name: string | null }) {
  const hue = hueFromDid(did)
  const initial = (name?.trim()[0] || '?').toUpperCase()
  return (
    <div
      className="flex h-16 w-16 shrink-0 items-center justify-center rounded-full text-2xl font-medium text-white"
      style={{ backgroundColor: `hsl(${hue}, 55%, 45%)` }}
    >
      {initial}
    </div>
  )
}

function hueFromDid(did: string): number {
  let h = 0
  for (let i = 0; i < did.length; i++) {
    h = (h * 31 + did.charCodeAt(i)) & 0xff_ff_ff
  }
  return h % 360
}
