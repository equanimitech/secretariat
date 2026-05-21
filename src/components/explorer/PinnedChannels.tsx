import { useCallback, useEffect, useState } from 'react'
import { Hash, Pin } from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { pinnedStore, type PinnedEntry } from './pinnedStore'
import { activeChannelStore } from './activeChannel'
import { OPEN_CHANNEL_EVENT, type OpenChannelRequest } from '@/components/layout/LeftSideBar'

interface PinnedChannelsProps {
  unreadByPath: Record<string, number>
}

/**
 * Slack-style starred section at the top of the sidebar. Flat list of
 * pinned channels with org-prefixed labels. Click → open channel tab
 * via the same event the tree dispatches. Right-click → unpin.
 *
 * Renders nothing when the pinned set is empty (no header chrome
 * either — keep the surface quiet by default).
 */
export function PinnedChannels({ unreadByPath }: PinnedChannelsProps) {
  const [entries, setEntries] = useState<PinnedEntry[]>(() => pinnedStore.list())
  const [activePath, setActivePath] = useState<string | null>(() =>
    activeChannelStore.get()
  )

  useEffect(() => pinnedStore.subscribe(() => setEntries(pinnedStore.list())), [])
  useEffect(
    () => activeChannelStore.subscribe(() => setActivePath(activeChannelStore.get())),
    []
  )

  if (entries.length === 0) return null

  return (
    <div className="shrink-0 border-b border-border bg-muted/10">
      <div className="px-3 pb-1 pt-2 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
        Pinned
      </div>
      <ul className="pb-1">
        {entries.map(entry => (
          <PinnedRow
            key={entry.path}
            entry={entry}
            unread={unreadByPath[entry.path] ?? 0}
            active={entry.path === activePath}
          />
        ))}
      </ul>
    </div>
  )
}

function PinnedRow({
  entry,
  unread,
  active,
}: {
  entry: PinnedEntry
  unread: number
  active: boolean
}) {
  const onActivate = useCallback(() => {
    window.dispatchEvent(
      new CustomEvent<OpenChannelRequest>(OPEN_CHANNEL_EVENT, {
        detail: {
          handle: entry.handle,
          name: entry.name,
          path: entry.path,
          org: entry.org,
        },
      })
    )
  }, [entry])

  // Active channels always count as read.
  const effectiveUnread = active ? 0 : unread
  const bold = effectiveUnread > 0

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <li
          role="button"
          tabIndex={0}
          onClick={onActivate}
          onKeyDown={e => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault()
              onActivate()
            }
          }}
          className={cn(
            'group flex h-6 cursor-pointer items-center gap-1.5 truncate px-3 text-[12px] hover:bg-accent/40',
            active && 'bg-accent text-accent-foreground'
          )}
          title={entry.path}
        >
          <Hash className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
          <span className={cn('truncate', bold && 'font-semibold')}>
            {entry.org ? (
              <>
                <span className="text-muted-foreground">{entry.org} / </span>
                {entry.name}
              </>
            ) : (
              entry.name
            )}
          </span>
          {effectiveUnread > 0 && <UnreadPill count={effectiveUnread} />}
        </li>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onSelect={() => pinnedStore.unpin(entry.path)}>
          <Pin className="h-3.5 w-3.5" />
          Unpin
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  )
}

function UnreadPill({ count }: { count: number }) {
  return (
    <span
      className="ml-auto shrink-0 rounded-full bg-muted px-1.5 py-0.5 text-[10px] font-medium text-foreground/70"
      title={`${count} unread`}
    >
      {count > 99 ? '99+' : count}
    </span>
  )
}
