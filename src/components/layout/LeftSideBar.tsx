import { useEffect, useRef, useState } from 'react'
import { cn } from '@/lib/utils'
import { ExplorerTree } from '@/components/explorer/ExplorerTree'
import { PinnedChannels } from '@/components/explorer/PinnedChannels'
import { useUnreadCounts } from '@/components/explorer/useUnreadCounts'
import { pinnedStore } from '@/components/explorer/pinnedStore'

interface LeftSideBarProps {
  className?: string
}

export interface OpenChannelRequest {
  handle: string
  name: string
  path: string
  org: string | null
}

export const OPEN_CHANNEL_EVENT = 'secretariat:open-channel'

export function LeftSideBar({ className }: LeftSideBarProps) {
  const ref = useRef<HTMLDivElement>(null)
  const treeHostRef = useRef<HTMLDivElement>(null)
  const [size, setSize] = useState({ width: 240, height: 400 })
  const { unreadByPath, registerPath } = useUnreadCounts()

  useEffect(() => {
    if (!treeHostRef.current) return
    const obs = new ResizeObserver(entries => {
      const r = entries[0]?.contentRect
      if (r && r.width > 0 && r.height > 0) {
        setSize({ width: r.width, height: r.height })
      }
    })
    obs.observe(treeHostRef.current)
    return () => obs.disconnect()
  }, [])

  // Register every pinned channel path so its unread count is tracked
  // even when the tree hasn't surfaced it yet.
  useEffect(() => {
    const seed = () => {
      for (const e of pinnedStore.list()) registerPath(e.path)
    }
    seed()
    return pinnedStore.subscribe(seed)
  }, [registerPath])

  return (
    <div
      ref={ref}
      className={cn(
        'flex h-full w-full flex-col overflow-hidden border-r bg-background',
        className
      )}
    >
      <PinnedChannels unreadByPath={unreadByPath} />
      <div ref={treeHostRef} className="min-h-0 flex-1">
        <ExplorerTree
          width={size.width}
          height={size.height}
          unreadByPath={unreadByPath}
          registerPath={registerPath}
          onOpenChannel={req => {
            window.dispatchEvent(
              new CustomEvent<OpenChannelRequest>(OPEN_CHANNEL_EVENT, {
                detail: req,
              })
            )
          }}
        />
      </div>
    </div>
  )
}
