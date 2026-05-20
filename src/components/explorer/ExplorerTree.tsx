import { useCallback, useEffect, useRef, useState } from 'react'
import { Tree, type NodeApi, type NodeRendererProps } from 'react-arborist'
import {
  ChevronRight,
  ChevronDown,
  FileText,
  Folder,
  FolderOpen,
  Hash,
  Lock,
  Building2,
  File as FileIcon,
  Terminal,
  Trash2,
} from 'lucide-react'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { toast } from 'sonner'
import { commands } from '@/lib/bindings'
import { cn } from '@/lib/utils'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { entryToNode, type ExplorerNode } from './types'

interface ExplorerTreeProps {
  width: number
  height: number
  /** Channel-leaf activation — adds a session tab. */
  onOpenChannel: (info: {
    handle: string
    name: string
    path: string
    org: string | null
  }) => void
}

export function ExplorerTree({ width, height, onOpenChannel }: ExplorerTreeProps) {
  const [data, setData] = useState<ExplorerNode[]>([])
  const [error, setError] = useState<string | null>(null)
  const loadingRef = useRef<Set<string>>(new Set())

  const refreshRoots = useCallback(() => {
    void commands.listExplorerRoots().then(res => {
      if (res.status === 'ok') {
        setData(res.data.map(e => entryToNode(e)))
      } else {
        setError(res.error)
      }
    })
  }, [])

  useEffect(() => {
    refreshRoots()
  }, [refreshRoots])

  const loadChildren = useCallback(async (node: ExplorerNode) => {
    if (loadingRef.current.has(node.path)) return
    if (node.children !== undefined) return
    loadingRef.current.add(node.path)
    try {
      const res = await commands.listDir(node.path)
      if (res.status !== 'ok') {
        setError(res.error)
        return
      }
      setData(prev => spliceChildren(prev, node.path, res.data.map(e => entryToNode(e, node.org))))
    } finally {
      loadingRef.current.delete(node.path)
    }
  }, [])

  const handleActivate = useCallback(
    (node: NodeApi<ExplorerNode>) => {
      const d = node.data
      if (d.kind === 'channel_leaf' && d.handle) {
        onOpenChannel({
          handle: d.handle,
          name: d.name,
          path: d.path,
          org: d.org,
        })
        return
      }
      if (d.kind === 'file' && d.ext === 'md') {
        window.dispatchEvent(
          new CustomEvent('secretariat:open-markdown', {
            detail: { path: d.path, name: d.name },
          })
        )
        return
      }
    },
    [onOpenChannel]
  )

  const handleToggle = useCallback(
    (id: string) => {
      const node = findNode(data, id)
      if (node && node.children === undefined) {
        void loadChildren(node)
      }
    },
    [data, loadChildren]
  )

  return (
    <div className="flex h-full w-full flex-col bg-background">
      {error && (
        <div className="border-b border-destructive/40 bg-destructive/10 px-3 py-1.5 text-[11px] text-destructive">
          {error}
        </div>
      )}
      <Tree<ExplorerNode>
        data={data}
        width={width}
        height={height}
        indent={14}
        rowHeight={26}
        openByDefault={false}
        onActivate={handleActivate}
        onToggle={handleToggle}
        disableDrag
        disableDrop
        disableMultiSelection
      >
        {makeNodeRenderer({ refreshRoots })}
      </Tree>
    </div>
  )
}

interface NodeContext {
  refreshRoots: () => void
}

function makeNodeRenderer(ctx: NodeContext) {
  function Renderer(props: NodeRendererProps<ExplorerNode>) {
    return <Node {...props} {...ctx} />
  }
  return Renderer
}

function Node({
  node,
  style,
  dragHandle,
  refreshRoots,
}: NodeRendererProps<ExplorerNode> & NodeContext) {
  const d = node.data
  const Icon = pickIcon(d, node.isOpen)
  const row = (
    <div
      ref={dragHandle}
      style={style}
      className={cn(
        'group flex h-full cursor-pointer items-center gap-1.5 truncate pr-2 text-[12px]',
        node.isSelected && 'bg-accent text-accent-foreground'
      )}
      onClick={() => node.activate()}
      onDoubleClick={() => node.toggle()}
    >
      <span
        className="flex h-5 w-5 shrink-0 items-center justify-center text-muted-foreground"
        onClick={e => {
          e.stopPropagation()
          if (d.hasChildren) node.toggle()
        }}
      >
        {d.hasChildren
          ? node.isOpen
            ? <ChevronDown className="h-3 w-3" />
            : <ChevronRight className="h-3 w-3" />
          : null}
      </span>
      <Icon className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
      <span className="truncate">{labelFor(d)}</span>
    </div>
  )

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{row}</ContextMenuTrigger>
      <ContextMenuContent>
        <NodeMenuItems node={d} refreshRoots={refreshRoots} />
      </ContextMenuContent>
    </ContextMenu>
  )
}

function NodeMenuItems({
  node,
  refreshRoots,
}: {
  node: ExplorerNode
  refreshRoots: () => void
}) {
  const onReveal = async () => {
    try {
      await revealItemInDir(node.path)
    } catch (e) {
      console.warn('revealItemInDir failed', e)
    }
  }
  const onLaunchClaude = async () => {
    const res = await commands.launchClaudeAt(node.path, null)
    if (res.status === 'error') {
      toast.error(`Launch Claude failed: ${res.error}`)
    }
  }
  const onDeleteChannel = async () => {
    if (!node.handle) return
    const confirmed = window.confirm(
      `Delete channel "${node.name}"?\n\nThis removes the channel's directory tree and every envelope inside it. Cannot be undone.`
    )
    if (!confirmed) return
    const res = await commands.deleteChannel(node.handle, node.org)
    if (res.status === 'error') {
      toast.error(`Delete failed: ${res.error}`)
      return
    }
    toast.success(`Channel "${node.name}" deleted`)
    refreshRoots()
  }

  if (node.kind === 'channel_leaf') {
    return (
      <>
        <ContextMenuItem onSelect={onLaunchClaude}>
          <Terminal className="h-3.5 w-3.5" />
          Launch Claude here
        </ContextMenuItem>
        <ContextMenuItem onSelect={onReveal}>
          <FolderOpen className="h-3.5 w-3.5" />
          Reveal in Finder
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem destructive onSelect={onDeleteChannel}>
          <Trash2 className="h-3.5 w-3.5" />
          Delete channel…
        </ContextMenuItem>
      </>
    )
  }
  if (node.kind === 'file') {
    return (
      <ContextMenuItem onSelect={onReveal}>
        <FolderOpen className="h-3.5 w-3.5" />
        Reveal in Finder
      </ContextMenuItem>
    )
  }
  // private | org | dir
  return (
    <ContextMenuItem onSelect={onReveal}>
      <FolderOpen className="h-3.5 w-3.5" />
      Reveal in Finder
    </ContextMenuItem>
  )
}

function labelFor(d: ExplorerNode): string {
  if (d.kind === 'channel_leaf' && d.handle) {
    return d.name
  }
  return d.name
}

function pickIcon(d: ExplorerNode, isOpen: boolean) {
  switch (d.kind) {
    case 'private':
      return Lock
    case 'org':
      return Building2
    case 'channel_leaf':
      return Hash
    case 'dir':
      return isOpen ? FolderOpen : Folder
    case 'file':
      return d.ext === 'md' ? FileText : FileIcon
    default:
      return FileIcon
  }
}

function spliceChildren(
  tree: ExplorerNode[],
  parentPath: string,
  children: ExplorerNode[]
): ExplorerNode[] {
  return tree.map(n => {
    if (n.path === parentPath) {
      return { ...n, children }
    }
    if (n.children && n.children.length > 0) {
      return { ...n, children: spliceChildren(n.children, parentPath, children) }
    }
    return n
  })
}

function findNode(tree: ExplorerNode[], id: string): ExplorerNode | null {
  for (const n of tree) {
    if (n.id === id) return n
    if (n.children && n.children.length > 0) {
      const inner = findNode(n.children, id)
      if (inner) return inner
    }
  }
  return null
}
