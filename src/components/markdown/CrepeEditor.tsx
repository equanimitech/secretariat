import { useEffect, useRef, useState } from 'react'
import { Crepe, CrepeFeature } from '@milkdown/crepe'
import '@milkdown/crepe/theme/common/style.css'
// Crepe ships paired stylesheets. We can't conditionally `import` CSS, so
// pull both as URL refs and inject the one matching the active theme via
// a managed <link> element. The wrong sheet would otherwise win the
// cascade and leave the editor stuck in one mode.
import frameLightUrl from '@milkdown/crepe/theme/frame.css?url'
import frameDarkUrl from '@milkdown/crepe/theme/frame-dark.css?url'

/// Watch the html `dark` class managed by ThemeProvider. Lets non-React
/// stylesheets (Crepe) follow the theme without needing useTheme everywhere.
function useHtmlDarkClass(): boolean {
  const [isDark, setIsDark] = useState(() =>
    document.documentElement.classList.contains('dark'),
  )
  useEffect(() => {
    const root = document.documentElement
    const sync = () => setIsDark(root.classList.contains('dark'))
    sync()
    const observer = new MutationObserver(sync)
    observer.observe(root, { attributes: true, attributeFilter: ['class'] })
    return () => observer.disconnect()
  }, [])
  return isDark
}

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
  const isDark = useHtmlDarkClass()

  useEffect(() => {
    onChangeRef.current = onChange
  })

  // Swap Crepe's theme sheet to match the html `dark` class. Single
  // <link>, replaced (not stacked) so the cascade stays deterministic.
  useEffect(() => {
    const link = document.createElement('link')
    link.rel = 'stylesheet'
    link.href = isDark ? frameDarkUrl : frameLightUrl
    link.dataset.crepeTheme = isDark ? 'dark' : 'light'
    document.head.appendChild(link)
    return () => {
      link.remove()
    }
  }, [isDark])

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
      // Envelopes are markdown text — no images, no media drops.
      features: {
        [CrepeFeature.ImageBlock]: false,
      },
      featureConfigs: {
        // Keep the slash-menu inside BlockEdit but hide the per-block
        // drag handle on the left — its drag interaction is glitchy in
        // Crepe 7.x and isn't worth the visual noise for our use.
        [CrepeFeature.BlockEdit]: {
          blockHandle: {
            shouldShow: () => false,
          },
        },
      },
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
