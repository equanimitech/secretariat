// Settings → About. App version + self-update via tauri-plugin-updater.

import { useCallback, useEffect, useState } from 'react'
import { getVersion } from '@tauri-apps/api/app'
import { Download, RefreshCw, CheckCircle2 } from 'lucide-react'
import { commands, type UpdateInfo } from '@/lib/bindings'
import { Label } from '@/components/ui/label'

type State =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'up-to-date' }
  | { kind: 'available'; update: UpdateInfo }
  | { kind: 'installing' }
  | { kind: 'error'; message: string }

export function AboutPane() {
  const [version, setVersion] = useState<string>('…')
  const [state, setState] = useState<State>({ kind: 'idle' })

  useEffect(() => {
    void getVersion().then(setVersion)
  }, [])

  const handleCheck = useCallback(async () => {
    setState({ kind: 'checking' })
    const result = await commands.checkForUpdate()
    if (result.status === 'error') {
      setState({ kind: 'error', message: result.error })
      return
    }
    if (result.data === null) {
      setState({ kind: 'up-to-date' })
      return
    }
    setState({ kind: 'available', update: result.data })
  }, [])

  const handleInstall = useCallback(async () => {
    setState({ kind: 'installing' })
    const result = await commands.installUpdate()
    if (result.status === 'error') {
      setState({ kind: 'error', message: result.error })
    }
    // On success the app restarts — no need to update state.
  }, [])

  return (
    <div className="space-y-6 p-2">
      <section className="space-y-3">
        <div>
          <Label className="text-sm font-medium">About Secretariat</Label>
          <p className="text-xs text-muted-foreground">
            Cryptographically attested AI-mediated correspondence.
          </p>
        </div>

        <div className="rounded-md border bg-muted/30 p-3 text-sm">
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground">Version</span>
            <code className="rounded bg-background px-1.5 py-0.5 text-xs">
              {version}
            </code>
          </div>
        </div>
      </section>

      <section className="space-y-3 border-t pt-4">
        <div>
          <Label className="text-sm font-medium">Updates</Label>
          <p className="text-xs text-muted-foreground">
            Check the release channel and install pending updates.
          </p>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handleCheck}
            disabled={state.kind === 'checking' || state.kind === 'installing'}
            className="inline-flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-sm hover:bg-muted disabled:opacity-50"
          >
            <RefreshCw
              className={`h-3.5 w-3.5 ${state.kind === 'checking' ? 'animate-spin' : ''}`}
            />
            {state.kind === 'checking' ? 'Checking…' : 'Check for updates'}
          </button>

          {state.kind === 'available' && (
            <button
              type="button"
              onClick={handleInstall}
              className="inline-flex items-center gap-1.5 rounded-md border bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:opacity-90"
            >
              <Download className="h-3.5 w-3.5" />
              Install {state.update.version} & restart
            </button>
          )}

          {state.kind === 'installing' && (
            <span className="text-xs text-muted-foreground">
              Downloading + installing — the app will restart…
            </span>
          )}
        </div>

        {state.kind === 'up-to-date' && (
          <div className="flex items-center gap-1.5 rounded-md border border-emerald-200 bg-emerald-50 p-2 text-xs text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950/40 dark:text-emerald-400">
            <CheckCircle2 className="h-3.5 w-3.5" />
            You're on the latest version.
          </div>
        )}

        {state.kind === 'available' && (
          <div className="rounded-md border bg-muted/30 p-3 text-xs">
            <div className="mb-1 font-medium">
              {state.update.version}{' '}
              {state.update.date && (
                <span className="text-muted-foreground">
                  · {state.update.date}
                </span>
              )}
            </div>
            {state.update.notes && (
              <pre className="whitespace-pre-wrap font-sans text-muted-foreground">
                {state.update.notes}
              </pre>
            )}
          </div>
        )}

        {state.kind === 'error' && (
          <div className="rounded-md border border-destructive bg-destructive/10 p-2 text-xs text-destructive">
            {state.message}
          </div>
        )}
      </section>
    </div>
  )
}
