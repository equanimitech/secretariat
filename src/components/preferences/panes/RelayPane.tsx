// Settings → Relay. The principal's relay endpoint(s) — the dumb pipe
// peers route envelopes through. Per the architectural invariant
// "Transports are adapters, not authorities," relays see signed
// ciphertext; never plaintext or contract terms. Most principals run
// against one shared relay (`secretariat.equanimi.tech`) today;
// self-hosted relays are a future-pitch path.

import { useCallback, useEffect, useState } from 'react'
import { Plus, CheckCircle2, Circle } from 'lucide-react'
import { commands } from '@/lib/bindings'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import type { RelayInfo } from '@/lib/bindings'

export function RelayPane() {
  const [relays, setRelays] = useState<RelayInfo[]>([])
  const [newEndpoint, setNewEndpoint] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [savedNote, setSavedNote] = useState<string | null>(null)

  const loadRelays = useCallback(async () => {
    const result = await commands.listRelays()
    if (result.status === 'ok') {
      setRelays(result.data)
    } else {
      setError(result.error)
    }
  }, [])

  useEffect(() => {
    void (async () => {
      const result = await commands.listRelays()
      if (result.status === 'ok') {
        setRelays(result.data)
      } else {
        setError(result.error)
      }
    })()
  }, [])

  const handleAdd = useCallback(async () => {
    const trimmed = newEndpoint.trim()
    if (!trimmed) {
      setError('Endpoint cannot be empty.')
      return
    }
    setBusy(true)
    setError(null)
    try {
      const result = await commands.addRelay(trimmed)
      if (result.status === 'error') {
        setError(result.error)
        return
      }
      setNewEndpoint('')
      setSavedNote('Added.')
      setTimeout(() => setSavedNote(null), 2000)
      await loadRelays()
    } finally {
      setBusy(false)
    }
  }, [newEndpoint, loadRelays])

  return (
    <div className="space-y-6 p-2">
      <section className="space-y-3">
        <div>
          <Label className="text-sm font-medium">Registered relays</Label>
          <p className="text-xs text-muted-foreground">
            Where your envelopes are routed in transit. Relays see signed
            ciphertext only — never plaintext, never contract terms.
          </p>
        </div>
        {relays.length === 0 ? (
          <p className="text-xs italic text-muted-foreground">
            No relays yet. Add one below — it&apos;ll register the first time
            you create or accept an invite against that endpoint.
          </p>
        ) : (
          <ul className="space-y-2">
            {relays.map(r => (
              <li
                key={r.endpoint}
                className="flex items-center gap-2 rounded-md border bg-muted/30 px-3 py-2"
              >
                {r.registered ? (
                  <CheckCircle2 className="h-4 w-4 shrink-0 text-emerald-600 dark:text-emerald-400" />
                ) : (
                  <Circle className="h-4 w-4 shrink-0 text-muted-foreground" />
                )}
                <code className="break-all text-xs flex-1">{r.endpoint}</code>
                <span className="text-xs text-muted-foreground">
                  {r.registered ? 'registered' : 'pending registration'}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="space-y-3 border-t pt-4">
        <div>
          <Label htmlFor="new-relay" className="text-sm font-medium">
            Add a relay
          </Label>
          <p className="text-xs text-muted-foreground">
            Paste the relay&apos;s HTTPS origin. Registration happens
            automatically the first time you invite or accept against it.
          </p>
        </div>
        <div className="flex items-center gap-2 max-w-md">
          <Input
            id="new-relay"
            type="url"
            placeholder="https://secretariat.example.com"
            value={newEndpoint}
            onChange={e => setNewEndpoint(e.target.value)}
            disabled={busy}
          />
          <button
            type="button"
            onClick={handleAdd}
            disabled={busy || !newEndpoint.trim()}
            className="inline-flex items-center gap-1.5 rounded-md border bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50"
          >
            <Plus className="h-3.5 w-3.5" />
            Add
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
    </div>
  )
}
