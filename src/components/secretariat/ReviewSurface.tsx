// Two-button home surface. The dashboard / inbox-columns / envelope-modal
// look from the v0.2.0 cut was too email-shaped — see
// `memory/project_two_buttons_home.md` for the design lock-in. The
// cadenced review-session walker that the buttons launch lands in a
// follow-up commit.
//
// Today's behavior: counts surface as ambient signal in the button
// labels; clicking either button is a no-op placeholder until the
// walker ships.

import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import {
  commands,
  type IdentityState,
  type Profile,
} from '@/lib/bindings'

export function ReviewSurface() {
  const [identity, setIdentity] = useState<IdentityState | null>(null)
  const [profile, setProfile] = useState<Profile | null>(null)
  const [inboxCount, setInboxCount] = useState<number>(0)
  const [queueCount, setQueueCount] = useState<number>(0)
  const [syncing, setSyncing] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setError(null)
    const [ident, prof, inbox, queue] = await Promise.all([
      commands.currentIdentity(),
      commands.getProfile(),
      commands.listInbox(),
      commands.listReviewQueue(),
    ])
    if (ident.status === 'ok') setIdentity(ident.data)
    if (prof.status === 'ok') setProfile(prof.data)
    if (inbox.status === 'ok') setInboxCount(inbox.data.length)
    else setError(inbox.error)
    if (queue.status === 'ok') setQueueCount(queue.data.length)
    else setError(queue.error)
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

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

  if (!identity) return null

  const displayName = profile?.display_name ?? null

  return (
    <div className="flex h-full flex-col items-center justify-center gap-12 bg-background p-8">
      <header className="flex flex-col items-center gap-3 text-center">
        <PrincipalAvatar did={identity.did} name={displayName} size="lg" />
        <p className="text-lg font-medium">{displayName ?? 'You'}</p>
      </header>

      <div className="flex flex-col gap-4 sm:flex-row">
        <ReviewButton
          label="Review inbox"
          count={inboxCount}
          onClick={() => copyPromptToClipboard('inbox', inboxCount)}
        />
        <ReviewButton
          label="Review outbox"
          count={queueCount}
          onClick={() => copyPromptToClipboard('outbox', queueCount)}
        />
      </div>

      <button
        type="button"
        onClick={handleSync}
        disabled={syncing}
        className="text-sm text-muted-foreground underline-offset-4 hover:underline disabled:opacity-50"
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

/// Copy a Claude-ready prompt to the clipboard. The cadenced in-app
/// walker is still future work (see `docs/ideas/two-buttons-cadenced-reviews.md`),
/// so the bridge today is: principal clicks button, prompt lands in
/// clipboard, paste into Claude Code/Desktop, Claude walks the queue
/// via the MCP tools.
async function copyPromptToClipboard(
  kind: 'inbox' | 'outbox',
  count: number
) {
  const prompt =
    kind === 'inbox'
      ? `Walk me through my Secretariat inbox. There ${
          count === 1 ? 'is 1 envelope' : `are ${count} envelopes`
        } waiting. Use the Secretariat MCP tools — list_inbox and read — to show me each one in turn. After each, ask if I want to reply.`
      : `Walk me through my Secretariat outbox queue. There ${
          count === 1 ? 'is 1 draft' : `are ${count} drafts`
        } awaiting review. Use the Secretariat MCP tools — list_review_queue and read — to show me each draft. For each, show the body, ask if I want to stamp it. Do not stamp without my explicit go.`
  try {
    await navigator.clipboard.writeText(prompt)
    toast.success(
      kind === 'inbox'
        ? 'Inbox-review prompt copied — paste into Claude'
        : 'Outbox-review prompt copied — paste into Claude'
    )
  } catch (err) {
    toast.error(`Could not copy to clipboard: ${String(err)}`)
  }
}

function ReviewButton({
  label,
  count,
  onClick,
}: {
  label: string
  count: number
  onClick: () => void
}) {
  const hasItems = count > 0
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex h-44 w-64 flex-col items-center justify-center gap-3 rounded-2xl border-2 bg-card p-6 transition hover:border-primary hover:shadow-md"
    >
      <span
        className={
          'h-3 w-3 rounded-full ' +
          (hasItems
            ? 'bg-amber-500 dark:bg-amber-400'
            : 'bg-emerald-500 dark:bg-emerald-400')
        }
        aria-hidden
      />
      <span className="text-xl font-semibold">{label}</span>
      <span className="text-sm text-muted-foreground">
        {hasItems
          ? count === 1
            ? '1 to review'
            : `${count} to review`
          : 'all clear'}
      </span>
    </button>
  )
}

function PrincipalAvatar({
  did,
  name,
  size,
}: {
  did: string
  name: string | null
  size: 'sm' | 'lg'
}) {
  const hue = hueFromDid(did)
  const initial = (name?.trim()[0] || '?').toUpperCase()
  const sizeClass =
    size === 'lg' ? 'h-16 w-16 text-2xl' : 'h-9 w-9 text-sm'
  return (
    <div
      className={`flex shrink-0 items-center justify-center rounded-full font-medium text-white ${sizeClass}`}
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
