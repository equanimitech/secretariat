import { useEffect, useRef, useState } from 'react'
import { cn } from '@/lib/utils'
import { ExplorerTree } from '@/components/explorer/ExplorerTree'

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
  const [size, setSize] = useState({ width: 240, height: 400 })

  useEffect(() => {
    if (!ref.current) return
    const obs = new ResizeObserver(entries => {
      const r = entries[0]?.contentRect
      if (r && r.width > 0 && r.height > 0) {
        setSize({ width: r.width, height: r.height })
      }
    })
    obs.observe(ref.current)
    return () => obs.disconnect()
  }, [])

  return (
    <div
      ref={ref}
      className={cn(
        'flex h-full w-full flex-col overflow-hidden border-r bg-background',
        className
      )}
    >
      <ExplorerTree
        width={size.width}
        height={size.height}
        onOpenChannel={req => {
          window.dispatchEvent(
            new CustomEvent<OpenChannelRequest>(OPEN_CHANNEL_EVENT, { detail: req })
          )
        }}
      />
    </div>
  )
}
