import { useEffect } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useUIStore } from '@/store/ui-store'
import { usePlatform } from './use-platform'

/**
 * Tracks the main window's fullscreen state and reflects it in two places:
 *
 * 1. `--app-corner-radius` (via the `.square-corners` class on the root) —
 *    rounded window corners only make sense when the window is *not* edge-
 *    to-edge with the screen. In fullscreen the OS clips at the display
 *    edge, so 12px rounded corners leave visible gaps along the bezel.
 *
 * 2. `useUIStore.isFullscreen` — consumed by titlebar / layout components
 *    that need to drop custom chrome (traffic lights, drag region) when
 *    macOS native fullscreen takes over the top edge.
 *
 * Rules:
 * - macOS: square corners + isFullscreen toggle whenever native fullscreen
 *   is entered/exited. The earlier "macOS always rounded" rule predated
 *   the macOS fullscreen support and dropped the corner-radius update,
 *   leaving visible gaps along the screen bezel.
 * - Windows: square when fullscreen (no rounded corners needed at screen edge)
 * - Linux: square when fullscreen
 */
export function useSquareCornersEffect() {
  const platform = usePlatform()
  const setSquareCorners = useUIStore(state => state.setSquareCorners)
  const setIsFullscreen = useUIStore(state => state.setIsFullscreen)

  useEffect(() => {
    let cancelled = false
    const window = getCurrentWindow()

    const updateCorners = async () => {
      const isFullscreen = await window.isFullscreen()
      if (cancelled) return
      setIsFullscreen(isFullscreen)
      // All platforms: square corners while fullscreen — at the screen
      // edge there's no surrounding chrome to soften.
      setSquareCorners(isFullscreen)
    }

    // Check initial state
    void updateCorners()

    // Listen for window state changes. `onResized` fires on macOS when
    // entering/exiting native fullscreen, which is what we need; we
    // don't need a separate fullscreen event.
    const unlisten = window.onResized(() => {
      if (cancelled) return
      void updateCorners()
    })

    return () => {
      cancelled = true
      void unlisten.then(fn => fn())
    }
  }, [platform, setSquareCorners, setIsFullscreen])
}
