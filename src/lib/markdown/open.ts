import { listen } from '@tauri-apps/api/event'
import { commands } from '@/lib/tauri-bindings'
import { logger } from '@/lib/logger'

async function drainAndOpenPending(): Promise<void> {
  const paths = await commands.takePendingOpens()
  for (const p of paths) {
    logger.info(`Opening markdown window for ${p}`)
    const res = await commands.openMarkdownWindow(p)
    if (res.status === 'error') {
      logger.warn(`openMarkdownWindow failed for ${p}: ${res.error}`)
    }
  }
}

export function watchPendingOpens(): () => void {
  // Drain once on startup — covers RunEvent::Opened that fired before this
  // listener attached (the documented Opened-before-Ready ordering trap).
  void drainAndOpenPending()

  let unlistenFn: (() => void) | undefined
  void listen('markdown://pending-opens-added', () => {
    void drainAndOpenPending()
  }).then(fn => {
    unlistenFn = fn
  })

  return () => {
    if (unlistenFn) unlistenFn()
  }
}
