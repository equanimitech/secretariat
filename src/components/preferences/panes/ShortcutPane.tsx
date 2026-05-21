// Settings → Shortcut. The quick-pane keyboard shortcut. The principal
// hits this hotkey from anywhere on the system to drop a capture
// without leaving their current focus. Defaults to ⌘⇧. on macOS.
//
// Save flow: write to preferences.json AND register the shortcut with
// the global-shortcut plugin in the same call. The plugin manages
// re-registration on app launch via `register_quick_pane_shortcut`.

import { useCallback, useEffect, useState } from 'react'
import { commands } from '@/lib/bindings'
import { Label } from '@/components/ui/label'
import { ShortcutPicker } from '../ShortcutPicker'

export function ShortcutPane() {
  const [shortcut, setShortcut] = useState<string | null>(null)
  const [defaultShortcut, setDefaultShortcut] = useState<string>('')
  const [busy, setBusy] = useState(false)
  const [savedNote, setSavedNote] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    void (async () => {
      const [prefs, def] = await Promise.all([
        commands.loadPreferences(),
        commands.getDefaultQuickPaneShortcut(),
      ])
      if (prefs.status === 'ok') {
        setShortcut(prefs.data.quick_pane_shortcut ?? null)
      }
      setDefaultShortcut(def)
    })()
  }, [])

  const handleChange = useCallback(async (next: string | null) => {
    setShortcut(next)
    setBusy(true)
    setError(null)
    setSavedNote(null)
    try {
      const prefs = await commands.loadPreferences()
      if (prefs.status === 'error') {
        setError(prefs.error)
        return
      }
      const updated = { ...prefs.data, quick_pane_shortcut: next }
      const save = await commands.savePreferences(updated)
      if (save.status === 'error') {
        setError(save.error)
        return
      }
      const apply = await commands.updateQuickPaneShortcut(next)
      if (apply.status === 'error') {
        setError(apply.error)
        return
      }
      setSavedNote('Saved.')
      setTimeout(() => setSavedNote(null), 2000)
    } finally {
      setBusy(false)
    }
  }, [])

  return (
    <div className="space-y-6 p-2">
      <section className="space-y-3">
        <div>
          <Label className="text-sm font-medium">Quick capture</Label>
          <p className="text-xs text-muted-foreground">
            Hit this from anywhere on the system to drop an idea or note into
            Secretariat without context-switching. The capture stays local —
            you&apos;ll review it at the next review session.
          </p>
        </div>
        <div className="flex items-center gap-2 max-w-sm">
          <ShortcutPicker
            value={shortcut}
            defaultValue={defaultShortcut}
            onChange={handleChange}
            disabled={busy}
          />
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
