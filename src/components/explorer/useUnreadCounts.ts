import { useCallback, useEffect, useRef, useState } from 'react'
import { commands } from '@/lib/bindings'
import { unreadStore } from './unreadState'

const ENVELOPE_OPENED_EVENT = 'secretariat:envelope-opened'

/**
 * Shared lazy unread-count tracker. The sidebar (tree + pinned strip)
 * both render unread badges over channel-dir paths; they consult this
 * hook to get a consistent map and to register new paths as they
 * appear in the UI.
 *
 * On registration we walk the channel-dir once via the existing
 * `list_envelopes_under` IPC, seed never-seen envelopes into the
 * `unreadStore` as "already opened" (anti-noise on first launch), then
 * count the remaining unread. When `secretariat:envelope-opened`
 * fires, every previously-registered path is recomputed.
 */
export function useUnreadCounts(): {
  unreadByPath: Record<string, number>
  registerPath: (path: string) => void
} {
  const [unreadByPath, setUnreadByPath] = useState<Record<string, number>>({})
  const counted = useRef<Set<string>>(new Set())

  const compute = useCallback(async (path: string, seedNew: boolean) => {
    const res = await commands.listEnvelopesUnder(path)
    if (res.status !== 'ok') return
    if (seedNew) {
      for (const p of res.data) {
        if (!unreadStore.wasSeenPreviously(p)) {
          unreadStore.markOpened(p)
        }
      }
    }
    const unread = res.data.filter(p => !unreadStore.isOpened(p)).length
    setUnreadByPath(prev =>
      prev[path] === unread ? prev : { ...prev, [path]: unread }
    )
  }, [])

  const registerPath = useCallback(
    (path: string) => {
      if (counted.current.has(path)) return
      counted.current.add(path)
      void compute(path, true)
    },
    [compute]
  )

  useEffect(() => {
    function onOpened() {
      for (const path of counted.current) {
        void compute(path, false)
      }
    }
    window.addEventListener(ENVELOPE_OPENED_EVENT, onOpened)
    return () => {
      window.removeEventListener(ENVELOPE_OPENED_EVENT, onOpened)
    }
  }, [compute])

  return { unreadByPath, registerPath }
}
