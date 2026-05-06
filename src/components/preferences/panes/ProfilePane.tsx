// Settings → Profile. The only Secretariat-shaped settings pane that
// ships in v0.2.x. Other panes (General/Appearance/Advanced) come from
// the template scaffold and are deliberately hidden from the navigation
// per `memory/project_settings_pane_shape.md` — kept on disk so we can
// repurpose them later, but not on the principal's surface.

import { useCallback, useEffect, useState } from 'react'
import { commands } from '@/lib/bindings'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

export function ProfilePane() {
  const [name, setName] = useState('')
  const [did, setDid] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [savedNote, setSavedNote] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    void (async () => {
      const [profile, identity] = await Promise.all([
        commands.getProfile(),
        commands.currentIdentity(),
      ])
      if (profile.status === 'ok' && profile.data) {
        setName(profile.data.display_name)
      }
      if (identity.status === 'ok' && identity.data) {
        setDid(identity.data.did)
      }
    })()
  }, [])

  const handleSave = useCallback(async () => {
    const trimmed = name.trim()
    if (!trimmed) {
      setError('Name cannot be empty.')
      return
    }
    setBusy(true)
    setSavedNote(null)
    setError(null)
    try {
      const result = await commands.setProfile(trimmed)
      if (result.status === 'error') {
        setError(result.error)
        return
      }
      setSavedNote('Saved.')
      setTimeout(() => setSavedNote(null), 2000)
    } finally {
      setBusy(false)
    }
  }, [name])

  return (
    <div className="space-y-6 p-2">
      <section className="space-y-3">
        <div>
          <Label htmlFor="profile-name" className="text-sm font-medium">
            Display name
          </Label>
          <p className="text-xs text-muted-foreground">
            How you appear to yourself in this app, and the suggested name
            others see when they claim your invites.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Input
            id="profile-name"
            value={name}
            onChange={e => setName(e.target.value)}
            placeholder="Your name"
            disabled={busy}
            className="max-w-sm"
          />
          <button
            type="button"
            onClick={handleSave}
            disabled={busy}
            className="rounded-md border bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50"
          >
            {busy ? 'Saving…' : 'Save'}
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
      </section>

      <section className="space-y-2 border-t pt-4">
        <Label className="text-sm font-medium">Identity</Label>
        <p className="text-xs text-muted-foreground">
          Your cryptographic DID. Generated on this device, never sent
          over the wire. Recipients use it to verify envelopes you stamp.
        </p>
        {did ? (
          <div className="flex items-center gap-2">
            <code className="break-all rounded bg-muted px-2 py-1 text-xs">
              {did}
            </code>
            <button
              type="button"
              onClick={() => navigator.clipboard.writeText(did)}
              className="rounded-md border px-2 py-1 text-xs hover:bg-muted"
            >
              Copy
            </button>
          </div>
        ) : (
          <p className="text-xs italic text-muted-foreground">
            No identity yet — finish onboarding first.
          </p>
        )}
      </section>

    </div>
  )
}
