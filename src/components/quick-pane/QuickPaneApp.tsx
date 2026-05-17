// QuickPaneApp — cmdk launcher with capture fallback.
//
// Type → matches against every launchable channel in the substrate
// (personal + every org's channel tree, with per-channel cognition
// overrides surfaced via a tiny badge). Enter on a Launch row fires
// `sec launch <handle> --org <alias>` semantics via the Tauri
// `launchChannelFromPane` command — applies the channel's
// `root_path` and `launch_env` (LM Studio routing, custom model args)
// transparently.
//
// Capture stays as the bottom row of the list: Enter when no Launch
// row is selected drops the typed text into `inbox:triage` via
// `quickCapture`, matching today's quick-pane muscle memory.
//
// Dismiss on submit, blur, or Escape — same window-management
// contract as the legacy single-input form.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen } from '@tauri-apps/api/event'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from '@/components/ui/command'
import {
  commands,
  type AppPreferences,
  type LaunchableChannel,
} from '@/lib/bindings'
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
  const [text, setText] = useState('')
  const [channels, setChannels] = useState<LaunchableChannel[]>([])
  const [prefs, setPrefs] = useState<AppPreferences | null>(null)
  const [busy, setBusy] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  // Theme + initial data on mount.
  useEffect(() => {
    applyTheme()
    void (async () => {
      const [chRes, prefRes] = await Promise.all([
        commands.listLaunchableChannels(),
        commands.loadPreferences(),
      ])
      if (chRes.status === 'ok') setChannels(chRes.data)
      if (prefRes.status === 'ok') setPrefs(prefRes.data)
    })()

    const unlistenTheme = listen('theme-changed', applyTheme)
    return () => {
      unlistenTheme.then(fn => fn())
    }
  }, [])

  // Focus on appear; dismiss on blur.
  useEffect(() => {
    const w = getCurrentWindow()
    const off = w.onFocusChanged(async ({ payload: focused }) => {
      if (focused) {
        applyTheme()
        // Refresh on every appearance — new channels may have shown up.
        void commands.listLaunchableChannels().then(r => {
          if (r.status === 'ok') setChannels(r.data)
        })
        inputRef.current?.focus()
      } else {
        await dismissQuickPane()
      }
    })
    return () => {
      off.then(fn => fn())
    }
  }, [])

  // Escape — dismiss without action.
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

  const handleLaunch = useCallback(
    async (channel: LaunchableChannel) => {
      if (busy) return
      setBusy(true)
      try {
        const result = await commands.launchChannelFromPane(
          channel.handle,
          channel.org,
          prefs?.assistant_terminal ?? null
        )
        if (result.status === 'error') {
          logger.error('Launch failed', { error: result.error })
        }
      } finally {
        setBusy(false)
        setText('')
        await dismissQuickPane()
      }
    },
    [busy, prefs]
  )

  const handleCapture = useCallback(async () => {
    if (busy) return
    const body = text.trim()
    if (!body) return
    setBusy(true)
    try {
      const result = await commands.quickCapture(body)
      if (result.status === 'error') {
        logger.error('Capture failed', { error: result.error })
      }
    } finally {
      setBusy(false)
      setText('')
      await dismissQuickPane()
    }
  }, [text, busy])

  // Filter channels by query — cmdk's built-in filter handles ranking;
  // we hand it raw rows and let it score against the user's typing.
  const channelRows = useMemo(() => channels, [channels])

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden rounded-[var(--app-corner-radius)] border border-border bg-background shadow-lg">
      <Command className="flex h-full flex-col" loop>
        <CommandInput
          ref={inputRef}
          value={text}
          onValueChange={setText}
          placeholder="Channel to launch or thought to capture…"
          autoFocus
        />
        <CommandList className="flex-1">
          <CommandEmpty>
            {/* When the user types but nothing matches, cmdk hides
                groups. We still want the capture row available — it's
                rendered as the always-visible bottom group below this
                section, and cmdk's empty state only fires when the
                Launch group has no matches. Keep this block lean. */}
            <div className="px-3 py-1.5 text-xs text-muted-foreground">
              No matching channels.
            </div>
          </CommandEmpty>

          {channelRows.length > 0 && (
            <CommandGroup heading="Launch">
              {channelRows.map(ch => (
                <CommandItem
                  key={`${ch.org ?? '_self'}/${ch.handle}`}
                  value={`${ch.handle} ${ch.org ?? ''} ${ch.name} ${ch.root_path}`}
                  onSelect={() => handleLaunch(ch)}
                  className="flex items-center gap-3"
                >
                  <span className="flex flex-1 flex-col overflow-hidden">
                    <span className="flex items-center gap-2">
                      <span className="font-medium">{ch.handle}</span>
                      {ch.has_cognition_override && (
                        <span
                          className="rounded-sm bg-amber-400/20 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-amber-700 dark:text-amber-300"
                          title="Per-channel cognition override (e.g. LM Studio)"
                        >
                          override
                        </span>
                      )}
                    </span>
                    <span className="truncate text-xs text-muted-foreground">
                      {ch.org ? `${ch.org} · ` : ''}
                      {ch.root_path}
                    </span>
                  </span>
                </CommandItem>
              ))}
            </CommandGroup>
          )}

          <CommandSeparator />

          <CommandGroup heading="Capture">
            <CommandItem
              value="__capture_fallback__"
              onSelect={() => void handleCapture()}
              keywords={text ? [text] : []}
              className="flex items-center gap-3"
            >
              <span className="flex flex-col">
                <span className="font-medium">
                  {text.trim()
                    ? `Capture "${text.trim().slice(0, 60)}${text.trim().length > 60 ? '…' : ''}"`
                    : 'Capture to inbox:triage'}
                </span>
                <span className="text-xs text-muted-foreground">
                  Falls back when no channel matches · saves locally, never sent
                </span>
              </span>
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </Command>
    </div>
  )
}
