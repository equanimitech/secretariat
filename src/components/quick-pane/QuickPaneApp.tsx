// QuickPaneApp — placeholder pane.
//
// The channel-launch + capture flow this pane used to host was removed in
// the git-native cut (its backing Tauri commands are gone). The pane
// window itself is still created by the Rust shell + global shortcut, so
// this component stays as a minimal, dismissable surface to keep that
// window valid. It carries no removed-command wiring.

import { useEffect } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { commands } from '@/lib/bindings'
import { logger } from '@/lib/logger'

async function dismissQuickPane() {
  const result = await commands.dismissQuickPane()
  if (result.status === 'error') {
    logger.error('Failed to dismiss quick pane', { error: result.error })
  }
}

function applyTheme() {
  const theme = localStorage.getItem('ui-theme') || 'system'
  const root = document.documentElement
  root.classList.remove('light', 'dark')
  if (theme === 'system') {
    const sys = window.matchMedia('(prefers-color-scheme: dark)').matches
      ? 'dark'
      : 'light'
    root.classList.add(sys)
  } else {
    root.classList.add(theme)
  }
}

export default function QuickPaneApp() {
  useEffect(() => {
    applyTheme()
    const w = getCurrentWindow()
    const off = w.onFocusChanged(async ({ payload: focused }) => {
      if (focused) {
        applyTheme()
      } else {
        await dismissQuickPane()
      }
    })
    return () => {
      off.then(fn => fn())
    }
  }, [])

  useEffect(() => {
    const onKey = async (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        await dismissQuickPane()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  return (
    <div className="flex h-screen w-screen items-center justify-center rounded-[var(--app-corner-radius)] border border-border bg-background p-6 text-center shadow-lg">
      <p className="text-sm text-muted-foreground">
        Open Secretariat to edit and stamp your markdown.
      </p>
    </div>
  )
}
