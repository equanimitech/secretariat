import { useEffect, useRef } from 'react'
import { Crepe } from '@milkdown/crepe'
import '@milkdown/crepe/theme/common/style.css'
import '@milkdown/crepe/theme/frame.css'

interface CrepeEditorProps {
  initialValue: string
  onChange: (markdown: string) => void
}

/**
 * React wrapper for Milkdown Crepe.
 *
 * Change detection: we poll `crepe.getMarkdown()` every 500ms instead of
 * using Crepe's listener API (`crepe.on(api => api.markdownUpdated(...))`)
 * because the listener plugin's `editorView` context isn't injected at
 * register-time in Crepe 7.x — registering a listener triggers a
 * `MilkdownError: Context "editorView" not found`. Polling sidesteps the
 * lifecycle issue, costs ~zero, and is good enough for autosave cadence.
 */
export function CrepeEditor({ initialValue, onChange }: CrepeEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const onChangeRef = useRef(onChange)
  const initialValueRef = useRef(initialValue)

  useEffect(() => {
    onChangeRef.current = onChange
  })

  useEffect(() => {
    const host = hostRef.current
    if (!host) return

    let alive = true
    let attached: Crepe | null = null
    let pollTimer: number | null = null
    let lastSeen = initialValueRef.current

    const crepe = new Crepe({
      root: host,
      defaultValue: initialValueRef.current,
    })

    crepe
      .create()
      .then(() => {
        if (!alive) {
          void crepe.destroy()
          return
        }
        attached = crepe
        pollTimer = window.setInterval(() => {
          if (!attached) return
          const md = attached.getMarkdown()
          if (md !== lastSeen) {
            lastSeen = md
            onChangeRef.current(md)
          }
        }, 500)
      })
      .catch(err => {
        console.error('Crepe create failed', err)
      })

    return () => {
      alive = false
      if (pollTimer !== null) window.clearInterval(pollTimer)
      if (attached) void attached.destroy()
    }
  }, [])

  return <div ref={hostRef} className="prose-host h-full overflow-auto" />
}
