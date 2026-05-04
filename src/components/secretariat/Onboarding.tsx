// Two-screen onboarding wizard. Pitch:
// `docs/pitches/2026-05-04-onboarding-wizard.md`. BCT/PDP review:
// `docs/pitches/2026-05-04-onboarding-wizard-bct-review.md`.
//
// Screen 1: name + identity (one button — set_profile + init_identity).
// Screen 2: optional connect (paste invite OR skip).
//
// No celebration animation, no progress bar, no streak. Closing the
// app mid-wizard preserves partial state — the App.tsx router brings
// the principal back to the right screen on next launch.

import { useCallback, useState } from 'react'
import { commands } from '@/lib/bindings'

type Phase =
  | { kind: 'identity' }
  | { kind: 'connect'; did: string; name: string }

interface OnboardingProps {
  onComplete: () => void
}

export function Onboarding({ onComplete }: OnboardingProps) {
  const [phase, setPhase] = useState<Phase>({ kind: 'identity' })

  return (
    <div className="flex h-full items-center justify-center bg-background p-8">
      <div className="w-full max-w-md space-y-6">
        {phase.kind === 'identity' ? (
          <IdentityStep
            onSetUp={(did, name) => setPhase({ kind: 'connect', did, name })}
          />
        ) : (
          <ConnectStep did={phase.did} name={phase.name} onDone={onComplete} />
        )}
      </div>
    </div>
  )
}

function IdentityStep({
  onSetUp,
}: {
  onSetUp: (did: string, name: string) => void
}) {
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault()
      const trimmed = name.trim()
      if (!trimmed) {
        setError('Tell us what to call you.')
        return
      }
      setBusy(true)
      setError(null)
      try {
        const profile = await commands.setProfile(trimmed)
        if (profile.status === 'error') {
          setError(profile.error)
          return
        }
        const identity = await commands.initIdentity()
        if (identity.status === 'error') {
          setError(identity.error)
          return
        }
        onSetUp(identity.data.did, profile.data.display_name)
      } finally {
        setBusy(false)
      }
    },
    [name, onSetUp]
  )

  return (
    <form onSubmit={handleSubmit} className="space-y-5">
      <header className="space-y-2">
        <h1 className="text-2xl font-semibold">Welcome to Secretariat.</h1>
        <p className="text-sm text-muted-foreground">
          Async generative communication for professionals, stamped by humans.
        </p>
      </header>

      <div className="space-y-2">
        <label
          htmlFor="onboarding-name"
          className="text-sm font-medium"
        >
          What should we call you?
        </label>
        <input
          id="onboarding-name"
          type="text"
          value={name}
          onChange={e => setName(e.target.value)}
          placeholder="Rafa"
          autoFocus
          autoComplete="off"
          spellCheck={false}
          disabled={busy}
          className="w-full rounded-md border bg-background px-3 py-2 text-base focus:outline-none focus:ring-2 focus:ring-ring"
        />
      </div>

      <p className="text-xs italic text-muted-foreground">
        Your identity will be generated on this device. Touch ID will protect
        every signature you make. Nothing leaves your Mac without your
        fingerprint.
      </p>

      {error && (
        <div className="rounded-md border border-destructive bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}

      <button
        type="submit"
        disabled={busy || !name.trim()}
        className="w-full rounded-md bg-primary px-4 py-2.5 font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50"
      >
        {busy ? 'Setting up…' : 'Set me up'}
      </button>
    </form>
  )
}

function ConnectStep({
  did,
  name,
  onDone,
}: {
  did: string
  name: string
  onDone: () => void
}) {
  const [inviteUrl, setInviteUrl] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [generatedInvite, setGeneratedInvite] = useState<string | null>(null)

  const handleClaim = useCallback(async () => {
    const url = inviteUrl.trim()
    if (!url) {
      setError('Paste an invite URL or pick another option.')
      return
    }
    setBusy(true)
    setError(null)
    try {
      const result = await commands.claimInviteUrl(url)
      if (result.status === 'ok') {
        onDone()
      } else {
        setError(result.error)
      }
    } finally {
      setBusy(false)
    }
  }, [inviteUrl, onDone])

  const handleCreateInvite = useCallback(async () => {
    setBusy(true)
    setError(null)
    try {
      const result = await commands.createInvite('first-contact')
      if (result.status === 'ok') {
        setGeneratedInvite(result.data)
        // Best-effort copy to clipboard. Tauri's clipboard plugin
        // requires a permission grant we'll add later; for now leave
        // the URL visible so the principal can copy manually.
      } else {
        setError(result.error)
      }
    } finally {
      setBusy(false)
    }
  }, [])

  return (
    <div className="space-y-5">
      <header className="space-y-3">
        <div className="flex items-center gap-3">
          <Avatar did={did} name={name} />
          <div>
            <p className="font-medium">{name}</p>
            <code className="block truncate text-xs text-muted-foreground">
              {did}
            </code>
          </div>
        </div>
        <h2 className="text-xl font-semibold">Connect with someone.</h2>
        <p className="text-sm text-muted-foreground">
          Paste an invite URL someone sent you, or generate one to share. You
          can also skip and do this later.
        </p>
      </header>

      <div className="space-y-2">
        <input
          type="url"
          value={inviteUrl}
          onChange={e => setInviteUrl(e.target.value)}
          placeholder="https://… /v0/invite/…   or   secretariat://…"
          autoComplete="off"
          spellCheck={false}
          disabled={busy}
          className="w-full rounded-md border bg-background px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
        />
        <button
          type="button"
          onClick={handleClaim}
          disabled={busy || !inviteUrl.trim()}
          className="w-full rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50"
        >
          {busy ? 'Claiming…' : 'Claim invite'}
        </button>
      </div>

      <div className="relative">
        <hr className="border-t" />
        <span className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 bg-background px-2 text-xs text-muted-foreground">
          or
        </span>
      </div>

      {generatedInvite ? (
        <div className="space-y-2 rounded-md border bg-muted p-3 text-xs">
          <p>Share this URL with someone:</p>
          <code className="block break-all select-all">{generatedInvite}</code>
          <p className="text-muted-foreground">
            Send it via iMessage, email, anywhere. They'll see a landing page
            with "Open in Secretariat" if they have the app, or an install
            link if they don't.
          </p>
        </div>
      ) : (
        <button
          type="button"
          onClick={handleCreateInvite}
          disabled={busy}
          className="w-full rounded-md border px-4 py-2 text-sm hover:bg-muted disabled:opacity-50"
        >
          {busy ? 'Generating…' : "I'll invite someone"}
        </button>
      )}

      {error && (
        <div className="rounded-md border border-destructive bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}

      <div className="flex gap-2">
        <button
          type="button"
          onClick={onDone}
          disabled={busy}
          className="flex-1 rounded-md border px-4 py-2 text-sm hover:bg-muted disabled:opacity-50"
        >
          Skip for now
        </button>
        <button
          type="button"
          onClick={onDone}
          disabled={busy || !generatedInvite}
          className="flex-1 rounded-md border px-4 py-2 text-sm hover:bg-muted disabled:opacity-50"
        >
          {generatedInvite ? "I'm done" : 'Continue'}
        </button>
      </div>
    </div>
  )
}

/// Deterministic avatar derived from DID hash + initials from display name.
/// HSL hue from the DID, fixed saturation+lightness for legibility. Same
/// principal always renders the same color, distinct enough across peers.
function Avatar({ did, name }: { did: string; name: string }) {
  const hue = hueFromDid(did)
  const initial = (name.trim()[0] || '?').toUpperCase()
  return (
    <div
      className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full font-medium text-white"
      style={{ backgroundColor: `hsl(${hue}, 55%, 45%)` }}
      title={did}
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
