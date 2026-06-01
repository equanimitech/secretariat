import { useCallback, useEffect, useMemo, useState } from 'react'
import { X, FileText } from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { MarkdownWindow } from '@/components/markdown/MarkdownWindow'
import { loadTabs, makeMarkdownTab, saveTabs } from './storage'
import type { PersistedTabs, Tab } from './types'

export const OPEN_MARKDOWN_EVENT = 'secretariat:open-markdown'
export interface OpenMarkdownRequest {
  path: string
  name: string
}

export function SessionTabs() {
  const [state, setState] = useState<PersistedTabs>(() => loadTabs())

  useEffect(() => {
    saveTabs(state)
  }, [state])

  const activeTab = useMemo(
    () => state.tabs.find(t => t.id === state.activeId) ?? null,
    [state]
  )

  // Sidebar tree → tab strip. Markdown files open (or refocus) a tab.
  useEffect(() => {
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
    window.addEventListener(
      OPEN_MARKDOWN_EVENT,
      onOpenMarkdown as EventListener
    )
    return () => {
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
      {state.tabs.length > 0 && (
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
          </div>
        </div>
      )}

      <div className="min-h-0 flex-1">
        {activeTab ? <TabBody tab={activeTab} /> : <EmptyState />}
      </div>
    </div>
  )
}

function TabBody({ tab }: { tab: Tab }) {
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
      <FileText className="h-3 w-3 shrink-0 opacity-60" />
      <span className="truncate max-w-[180px]">{tab.name}</span>
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
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{row}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onSelect={() => onClose()}>
          <X className="h-3.5 w-3.5" />
          Close tab
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  )
}

function EmptyState() {
  return (
    <div className="flex h-full w-full items-center justify-center">
      <div className="flex flex-col items-center gap-3 text-center">
        <p className="text-sm text-muted-foreground">
          Open a markdown file from the sidebar to start editing.
        </p>
        <p className="font-mono text-[10px] text-muted-foreground">
          ⌘W close · ⌘1..9 jump
        </p>
      </div>
    </div>
  )
}
