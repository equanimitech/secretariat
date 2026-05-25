// Two-screen onboarding:
// 1. Identity — name + did:key generation
// 2. Cognition provider — choose a scribe substrate (today: Claude Code) or skip
//
// Closing mid-wizard preserves partial state — App.tsx router brings
// the principal back here on next launch if identity is missing OR if
// the principal has not yet decided about scribes.

import { useCallback, useEffect, useState } from 'react'
import { commands } from '@/lib/bindings'

interface OnboardingProps {
  onComplete: () => void
}

type Step = 'identity' | 'scribe'

export function Onboarding({ onComplete }: OnboardingProps) {
  const [step, setStep] = useState<Step>('identity')

  return (
    <div className="flex h-full items-center justify-center bg-background p-8">
      <div className="w-full max-w-md space-y-6">
        {step === 'identity' && (
          <IdentityStep onDone={() => setStep('scribe')} />
        )}
        {step === 'scribe' && <ScribeStep onDone={onComplete} />}
      </div>
    </div>
  )
}

function IdentityStep({ onDone }: { onDone: () => void }) {
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
        const identity = await commands.initIdentity()
        if (identity.status === 'error') {
          setError(identity.error)
          return
        }
        const profile = await commands.setProfile(trimmed)
        if (profile.status === 'error') {
          setError(profile.error)
          return
        }
        onDone()
      } finally {
        setBusy(false)
      }
    },
    [name, onDone]
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
        <label htmlFor="onboarding-name" className="text-sm font-medium">
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
        {busy ? 'Setting up…' : 'Continue'}
      </button>
    </form>
  )
}

// ---------------------------------------------------------------------------
// Scribe step — cognition provider selection
// ---------------------------------------------------------------------------

type Substrate = 'claude-code'

interface SubstrateOption {
  id: Substrate
  label: string
  description: string
  recommended?: boolean
}

const SUBSTRATES: SubstrateOption[] = [
  {
    id: 'claude-code',
    label: 'Claude Code',
    description:
      'Claude composes envelopes on your behalf via the bundled MCP server. Drafts arrive in your queue for review and Touch ID stamp.',
    recommended: true,
  },
]

function ScribeStep({ onDone }: { onDone: () => void }) {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [existingScribe, setExistingScribe] = useState<string | null>(null)

  // If returning to onboarding with a scribe already provisioned, skip
  // ahead transparently — don't ask the principal to choose again.
  useEffect(() => {
    let cancelled = false
    void (async () => {
      const result = await commands.listScribes()
      if (cancelled || result.status === 'error') {
        return
      }
      const scribes = result.data
      if (scribes.length > 0 && scribes[0]) {
        setExistingScribe(scribes[0].name)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  const handleProvision = useCallback(
    async (substrate: Substrate) => {
      setBusy(true)
      setError(null)
      try {
        const result = await commands.provisionScribe('claude', substrate)
        if (result.status === 'error') {
          setError(result.error)
          return
        }
        onDone()
      } finally {
        setBusy(false)
      }
    },
    [onDone]
  )

  const handleSkip = useCallback(() => {
    onDone()
  }, [onDone])

  if (existingScribe) {
    return (
      <div className="space-y-5">
        <header className="space-y-2">
          <h1 className="text-2xl font-semibold">Scribe already set up.</h1>
          <p className="text-sm text-muted-foreground">
            <code>{existingScribe}</code> is wired as your scribe. You can
            manage scribes anytime from preferences.
          </p>
        </header>
        <button
          onClick={onDone}
          className="w-full rounded-md bg-primary px-4 py-2.5 font-medium text-primary-foreground hover:opacity-90"
        >
          Continue
        </button>
      </div>
    )
  }

  return (
    <div className="space-y-5">
      <header className="space-y-2">
        <h1 className="text-2xl font-semibold">Add a scribe?</h1>
        <p className="text-sm text-muted-foreground">
          A scribe drafts envelopes on your behalf. You always stamp the ones
          worth elevating to authoritative; ambient drafts stay signed-only. You
          can add or remove scribes anytime.
        </p>
      </header>

      <div className="space-y-3">
        {SUBSTRATES.map(sub => (
          <button
            key={sub.id}
            onClick={() => handleProvision(sub.id)}
            disabled={busy}
            className="w-full rounded-md border border-border bg-card p-4 text-left transition-colors hover:bg-accent disabled:opacity-50"
          >
            <div className="flex items-center justify-between">
              <span className="font-medium">{sub.label}</span>
              {sub.recommended && (
                <span className="text-xs uppercase tracking-wide text-muted-foreground">
                  Recommended
                </span>
              )}
            </div>
            <p className="mt-1 text-xs text-muted-foreground">
              {sub.description}
            </p>
          </button>
        ))}
      </div>

      <p className="text-xs italic text-muted-foreground">
        The scribe gets its own keypair, stored only on this device. Every
        envelope it composes carries the scribe&apos;s signature — verifiable as
        authorized by you, distinct from your own Touch-ID stamps.
      </p>

      {error && (
        <div className="rounded-md border border-destructive bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}

      <button
        onClick={handleSkip}
        disabled={busy}
        className="w-full rounded-md border border-border px-4 py-2 text-sm text-muted-foreground hover:bg-accent disabled:opacity-50"
      >
        {busy ? 'Provisioning…' : 'Skip — I’ll compose manually'}
      </button>
    </div>
  )
}
