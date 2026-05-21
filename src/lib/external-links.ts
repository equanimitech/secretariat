import { openUrl } from '@tauri-apps/plugin-opener'
import { logger } from './logger'

// Tauri webviews navigate in-window on plain `<a>` clicks, which traps the
// user inside the app shell. Anything addressing an external resource —
// http(s), mailto, tel — should hand off to the OS so it opens in the
// system browser / mail client. Anchors with non-external protocols
// (e.g. `secretariat:`, `file:`, `#fragment`) are left alone.
const EXTERNAL_PROTOCOLS = new Set(['http:', 'https:', 'mailto:', 'tel:'])

let installed = false

export function installExternalLinkHandler(): void {
  if (installed) return
  installed = true

  document.addEventListener(
    'click',
    event => {
      if (event.defaultPrevented) return
      if (event.button !== 0) return

      const target = event.target
      if (!(target instanceof Element)) return
      const anchor = target.closest('a')
      if (!anchor) return

      const href = anchor.getAttribute('href')
      if (!href || href.startsWith('#')) return

      let parsed: URL
      try {
        parsed = new URL(href, window.location.href)
      } catch {
        return
      }
      if (!EXTERNAL_PROTOCOLS.has(parsed.protocol)) return

      event.preventDefault()
      event.stopPropagation()
      void openUrl(parsed.toString()).catch(err => {
        logger.warn('openUrl failed', {
          error: String(err),
          href: parsed.toString(),
        })
      })
    },
    true
  )
}
