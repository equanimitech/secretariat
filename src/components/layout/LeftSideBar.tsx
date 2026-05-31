import { useEffect, useRef, useState } from 'react'
import { cn } from '@/lib/utils'
import { ExplorerTree } from '@/components/explorer/ExplorerTree'

interface LeftSideBarProps {
  className?: string
}

export function LeftSideBar({ className }: LeftSideBarProps) {
  const ref = useRef<HTMLDivElement>(null)
  const treeHostRef = useRef<HTMLDivElement>(null)
  const [size, setSize] = useState({ width: 240, height: 400 })

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

  return (
    <div
      ref={ref}
      className={cn(
        'flex h-full w-full flex-col overflow-hidden border-r bg-background',
        className
      )}
    >
      <div ref={treeHostRef} className="min-h-0 flex-1">
        <ExplorerTree width={size.width} height={size.height} />
      </div>
    </div>
  )
}
