// Settings → Paths. Surfaces the principal's `~/.secretariat/` location
// and offers a Reveal-in-Finder so they can poke around if they need to
// (recover a draft, eyeball the contact book, etc). Read-only display
// for now — making the path overridable would require a restart hook
// that the v0.3 substrate work hasn't shipped yet.

import { useEffect, useState } from 'react'
import { FolderOpen } from 'lucide-react'
import { commands } from '@/lib/bindings'
import { Label } from '@/components/ui/label'

export function PathsPane() {
  const [root, setRoot] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [revealing, setRevealing] = useState(false)

  useEffect(() => {
    void (async () => {
      const result = await commands.secretariatRoot()
      if (result.status === 'ok') {
        setRoot(result.data)
      } else {
        setError(result.error)
      }
    })()
  }, [])

  const handleReveal = async () => {
    if (!root) return
    setRevealing(true)
    try {
      const result = await commands.revealInFinder(root)
      if (result.status === 'error') {
        setError(result.error)
      }
    } finally {
      setRevealing(false)
    }
  }

  return (
    <div className="space-y-6 p-2">
      <section className="space-y-3">
        <div>
          <Label className="text-sm font-medium">Secretariat home</Label>
          <p className="text-xs text-muted-foreground">
            Where your keys, contacts, drafts, and inbox live on disk.
            Everything Secretariat does happens inside this folder — nothing
            is stored on a server.
          </p>
        </div>
        {root ? (
          <div className="flex items-center gap-2">
            <code className="block break-all rounded bg-muted px-2 py-1 text-xs flex-1 max-w-md">
              {root}
            </code>
            <button
              type="button"
              onClick={handleReveal}
              disabled={revealing}
              className="inline-flex items-center gap-1.5 rounded-md border px-2 py-1.5 text-xs hover:bg-muted disabled:opacity-50"
            >
              <FolderOpen className="h-3.5 w-3.5" />
              {revealing ? 'Opening…' : 'Reveal in Finder'}
            </button>
          </div>
        ) : (
          <p className="text-xs italic text-muted-foreground">
            Resolving path…
          </p>
        )}
        {error && (
          <div className="rounded-md border border-destructive bg-destructive/10 p-2 text-sm text-destructive">
            {error}
          </div>
        )}
      </section>

      <section className="space-y-2 border-t pt-4 text-xs text-muted-foreground">
        <p>
          Changing the home location requires editing the substrate&apos;s
          discovery logic and restarting. Reach for the CLI (
          <code className="rounded bg-muted px-1">SEC_HOME</code>{' '}
          environment variable) if you need to relocate during a migration.
        </p>
      </section>
    </div>
  )
}
