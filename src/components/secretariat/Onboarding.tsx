// Single-screen onboarding: name + identity. Connection ceremony
// (peer invite, org join) belongs in steady-state UI, not first-launch.
//
// Closing mid-wizard preserves partial state — App.tsx router brings
// the principal back here on next launch if identity is missing.

import { useCallback, useState } from 'react'
import { commands } from '@/lib/bindings'

interface OnboardingProps {
  onComplete: () => void
}

export function Onboarding({ onComplete }: OnboardingProps) {
  return (
    <div className="flex h-full items-center justify-center bg-background p-8">
      <div className="w-full max-w-md space-y-6">
        <IdentityStep onSetUp={onComplete} />
      </div>
    </div>
  )
}

function IdentityStep({ onSetUp }: { onSetUp: () => void }) {
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
        onSetUp()
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
        {busy ? 'Setting up…' : 'Set me up'}
      </button>
    </form>
  )
}
