import { useEffect, useRef, useState } from 'react'
import { Crepe, CrepeFeature } from '@milkdown/crepe'
import { editorViewCtx } from '@milkdown/kit/core'
import { $prose } from '@milkdown/kit/utils'
import type { EditorView } from '@milkdown/kit/prose/view'
import { search } from 'prosemirror-search'
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
    document.documentElement.classList.contains('dark')
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
  /** Render the document read-only (Attend intent): identical typography,
   * no caret, no edits, no change-poll. */
  readonly?: boolean
  /** Hands the live ProseMirror view up so the find bar can drive the
   * search plugin. Called with null when the editor tears down. */
  onViewReady?: (view: EditorView | null) => void
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
export function CrepeEditor({
  initialValue,
  onChange,
  readonly = false,
  onViewReady,
}: CrepeEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const onChangeRef = useRef(onChange)
  const onViewReadyRef = useRef(onViewReady)
  const initialValueRef = useRef(initialValue)
  const readonlyRef = useRef(readonly)
  const isDark = useHtmlDarkClass()

  useEffect(() => {
    onChangeRef.current = onChange
    onViewReadyRef.current = onViewReady
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
      // Envelopes are markdown text — no images, no media drops. BlockEdit
      // (the per-line +/drag handles) is disabled: drag-drop is glitchy in
      // Crepe 7.x and the handles forced an awkward left gutter. Writers use
      // markdown syntax directly; the body sits flush with no handle column.
      features: {
        [CrepeFeature.ImageBlock]: false,
        [CrepeFeature.BlockEdit]: false,
      },
    })

    // Crepe ships no find feature. `prosemirror-search` supplies the query
    // state, the match decorations and the next/prev commands; FindBar
    // drives it. Registered before create() — the underlying Milkdown
    // editor exists from the Crepe constructor onward.
    crepe.editor.use($prose(() => search()))

    crepe
      .create()
      .then(() => {
        if (!alive) {
          void crepe.destroy()
          return
        }
        attached = crepe
        // Read-only documents are still searchable, so the view goes up
        // before the readonly branch returns.
        crepe.editor.action(ctx => {
          onViewReadyRef.current?.(ctx.get(editorViewCtx))
        })
        if (readonlyRef.current) {
          // Sealed / read-only: lock the surface, never poll for edits.
          crepe.setReadonly(true)
          return
        }
        // Baseline to Crepe's OWN serialization. Crepe normalizes markdown
        // on load (whitespace, list markers, etc.), so getMarkdown() differs
        // from the on-disk text even with zero edits. Without this baseline
        // the first poll fires a phantom onChange — rewriting the file on
        // open and, on a sealed doc, looping the break-seal dialog.
        lastSeen = crepe.getMarkdown()
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
      onViewReadyRef.current?.(null)
      if (pollTimer !== null) window.clearInterval(pollTimer)
      if (attached) void attached.destroy()
    }
  }, [])

  // No own overflow — the parent (.flex-1.overflow-y-auto) scrolls. If this
  // host clips, the block handles overflowing into the left padding get cut
  // off; letting the parent own scroll keeps them visible.
  return <div ref={hostRef} className="prose-host" />
}
