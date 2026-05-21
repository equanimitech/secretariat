import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  Plus,
  X,
  Hash,
  FileText,
  Archive,
  ArchiveRestore,
} from 'lucide-react'
import { toast } from 'sonner'
import type { LaunchableChannel } from '@/lib/bindings'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { classifyEnvelopePath } from '@/lib/envelope-path'
import {
  OPEN_CHANNEL_EVENT,
  type OpenChannelRequest,
} from '@/components/layout/LeftSideBar'
import { unreadStore } from '@/components/explorer/unreadState'
import { activeChannelStore } from '@/components/explorer/activeChannel'
import { commands } from '@/lib/bindings'
import { MarkdownWindow } from '@/components/markdown/MarkdownWindow'
import { ChannelPicker } from './ChannelPicker'
import { ChannelTimeline } from './ChannelTimeline'
import { loadTabs, makeChannelTab, makeMarkdownTab, saveTabs } from './storage'
import type { PersistedTabs, Tab } from './types'

export const OPEN_MARKDOWN_EVENT = 'secretariat:open-markdown'
export interface OpenMarkdownRequest {
  path: string
  name: string
}

export function SessionTabs() {
  const [state, setState] = useState<PersistedTabs>(() => loadTabs())
  const [pickerOpen, setPickerOpen] = useState(false)

  useEffect(() => {
    saveTabs(state)
  }, [state])

  const activeTab = useMemo(
    () => state.tabs.find(t => t.id === state.activeId) ?? null,
    [state]
  )

  // Mirror the active channel-tab path into the shared store so the
  // explorer can suppress unread badges + bold styling on the channel
  // the principal is actively viewing.
  useEffect(() => {
    if (activeTab && activeTab.kind === 'channel') {
      activeChannelStore.set(activeTab.channelPath)
    } else {
      activeChannelStore.set(null)
    }
  }, [activeTab])

  const openChannelFromPicker = useCallback((channel: LaunchableChannel) => {
    void markChannelRead(channel.root_path)
    const tab = makeChannelTab({
      handle: channel.handle,
      name: channel.name,
      rootPath: channel.root_path,
      org: channel.org,
    })
    setState(prev => ({
      tabs: [...prev.tabs, tab],
      activeId: tab.id,
    }))
  }, [])

  // Sidebar tree → tab strip. Channel tabs refocus existing; new
  // otherwise.
  useEffect(() => {
    function onOpenChannel(e: Event) {
      const detail = (e as CustomEvent<OpenChannelRequest>).detail
      void markChannelRead(detail.path)
      setState(prev => {
        const existing = prev.tabs.find(
          t => t.kind === 'channel' && t.channelPath === detail.path
        )
        if (existing) return { ...prev, activeId: existing.id }
        const tab = makeChannelTab({
          handle: detail.handle,
          name: detail.name,
          rootPath: detail.path,
          org: detail.org,
        })
        return { tabs: [...prev.tabs, tab], activeId: tab.id }
      })
    }
    function onOpenMarkdown(e: Event) {
      const detail = (e as CustomEvent<OpenMarkdownRequest>).detail
      setState(prev => {
        const existing = prev.tabs.find(
          t => t.kind === 'markdown' && t.filePath === detail.path
        )
        if (existing) return { ...prev, activeId: existing.id }
        const tab = makeMarkdownTab({ path: detail.path, name: detail.name })
        return { tabs: [...prev.tabs, tab], activeId: tab.id }
      })
    }
    window.addEventListener(OPEN_CHANNEL_EVENT, onOpenChannel as EventListener)
    window.addEventListener(
      OPEN_MARKDOWN_EVENT,
      onOpenMarkdown as EventListener
    )
    return () => {
      window.removeEventListener(
        OPEN_CHANNEL_EVENT,
        onOpenChannel as EventListener
      )
      window.removeEventListener(
        OPEN_MARKDOWN_EVENT,
        onOpenMarkdown as EventListener
      )
    }
  }, [])

  const closeTab = useCallback((id: string) => {
    setState(prev => {
      const tabs = prev.tabs.filter(t => t.id !== id)
      const activeId =
        prev.activeId === id
          ? (tabs[tabs.length - 1]?.id ?? null)
          : prev.activeId
      return { tabs, activeId }
    })
  }, [])

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === 't') {
        e.preventDefault()
        setPickerOpen(true)
        return
      }
      if ((e.metaKey || e.ctrlKey) && e.key === 'w' && activeTab) {
        e.preventDefault()
        closeTab(activeTab.id)
        return
      }
      if ((e.metaKey || e.ctrlKey) && /^[1-9]$/.test(e.key)) {
        const idx = parseInt(e.key, 10) - 1
        const target = state.tabs[idx]
        if (target) {
          e.preventDefault()
          setState(prev => ({ ...prev, activeId: target.id }))
        }
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [state.tabs, activeTab, closeTab])

  return (
    <div className="flex h-full w-full flex-col bg-background">
      <div className="flex h-9 shrink-0 items-center border-b border-border bg-muted/30">
        <div className="flex flex-1 items-center gap-px overflow-x-auto">
          {state.tabs.map(tab => (
            <TabHeader
              key={tab.id}
              tab={tab}
              active={tab.id === state.activeId}
              onActivate={() =>
                setState(prev => ({ ...prev, activeId: tab.id }))
              }
              onClose={() => closeTab(tab.id)}
            />
          ))}
          <Button
            variant="ghost"
            size="sm"
            className="ml-1 h-7 px-2"
            onClick={() => setPickerOpen(true)}
            aria-label="Open channel"
          >
            <Plus className="h-4 w-4" />
          </Button>
        </div>
      </div>

      <div className="min-h-0 flex-1">
        {activeTab ? (
          <TabBody tab={activeTab} />
        ) : (
          <EmptyState onOpen={() => setPickerOpen(true)} />
        )}
      </div>

      <ChannelPicker
        open={pickerOpen}
        onOpenChange={setPickerOpen}
        onPick={openChannelFromPicker}
      />
    </div>
  )
}

function TabBody({ tab }: { tab: Tab }) {
  if (tab.kind === 'channel') {
    return <ChannelTimeline key={tab.id} tab={tab} />
  }
  return <MarkdownWindow key={tab.id} filePath={tab.filePath} embedded />
}

function TabHeader({
  tab,
  active,
  onActivate,
  onClose,
}: {
  tab: Tab
  active: boolean
  onActivate: () => void
  onClose: () => void
}) {
  const Icon = tab.kind === 'channel' ? Hash : FileText
  const label =
    tab.kind === 'channel'
      ? `${tab.org ? `${tab.org} / ` : ''}${tab.channelName}`
      : tab.name
  const row = (
    <div
      role="tab"
      aria-selected={active}
      onClick={onActivate}
      className={cn(
        'group flex h-7 cursor-pointer items-center gap-2 border-r border-border px-3 text-xs',
        active
          ? 'bg-background font-medium text-foreground'
          : 'text-muted-foreground hover:bg-background/50'
      )}
    >
      <Icon className="h-3 w-3 shrink-0 opacity-60" />
      <span className="truncate max-w-[180px]">{label}</span>
      <button
        type="button"
        aria-label="Close tab"
        onClick={e => {
          e.stopPropagation()
          onClose()
        }}
        className="opacity-40 transition-opacity hover:opacity-100"
      >
        <X className="h-3 w-3" />
      </button>
    </div>
  )
  if (tab.kind !== 'markdown') return row
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{row}</ContextMenuTrigger>
      <ContextMenuContent>
        <MarkdownTabMenuItems filePath={tab.filePath} onClose={onClose} />
      </ContextMenuContent>
    </ContextMenu>
  )
}

function MarkdownTabMenuItems({
  filePath,
  onClose,
}: {
  filePath: string
  onClose: () => void
}) {
  const { isEnvelope, isArchived } = classifyEnvelopePath(filePath)
  const onArchive = async () => {
    const res = await commands.archiveInboxEnvelope(filePath)
    if (res.status === 'error') {
      toast.error(`Archive failed: ${res.error}`)
      return
    }
    toast.success('Archived')
    onClose()
  }
  const onUnarchive = async () => {
    const res = await commands.unarchiveInboxEnvelope(filePath)
    if (res.status === 'error') {
      toast.error(`Unarchive failed: ${res.error}`)
      return
    }
    toast.success('Unarchived')
    onClose()
  }
  return (
    <>
      <ContextMenuItem onSelect={() => onClose()}>
        <X className="h-3.5 w-3.5" />
        Close tab
      </ContextMenuItem>
      {isArchived && (
        <ContextMenuItem onSelect={onUnarchive}>
          <ArchiveRestore className="h-3.5 w-3.5" />
          Unarchive
        </ContextMenuItem>
      )}
      {isEnvelope && !isArchived && (
        <ContextMenuItem onSelect={onArchive}>
          <Archive className="h-3.5 w-3.5" />
          Archive
        </ContextMenuItem>
      )}
    </>
  )
}

/**
 * Mark every envelope under a channel-dir as opened. Fires the
 * envelope-opened event so the explorer recomputes ancestor unread
 * counts.
 */
async function markChannelRead(channelPath: string) {
  const res = await commands.listEnvelopesUnder(channelPath)
  if (res.status !== 'ok') return
  let touched = false
  for (const p of res.data) {
    if (!unreadStore.isOpened(p)) {
      unreadStore.markOpened(p)
      touched = true
    }
  }
  if (touched) {
    window.dispatchEvent(new CustomEvent('secretariat:envelope-opened'))
  }
}

function EmptyState({ onOpen }: { onOpen: () => void }) {
  return (
    <div className="flex h-full w-full items-center justify-center">
      <div className="flex flex-col items-center gap-3 text-center">
        <p className="text-sm text-muted-foreground">No sessions open.</p>
        <Button onClick={onOpen}>
          <Plus className="mr-1 h-4 w-4" />
          Open channel
        </Button>
        <p className="font-mono text-[10px] text-muted-foreground">
          ⌘T new · ⌘W close · ⌘1..9 jump
        </p>
      </div>
    </div>
  )
}
