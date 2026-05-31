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
  Pencil,
  RefreshCw,
} from 'lucide-react'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { toast } from 'sonner'
import { commands } from '@/lib/bindings'
import { cn } from '@/lib/utils'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { entryToNode, type ExplorerNode } from './types'

interface ExplorerTreeProps {
  width: number
  height: number
}

export function ExplorerTree({ width, height }: ExplorerTreeProps) {
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
      const newChildren = res.data.map(e => entryToNode(e))
      setData(prev => spliceChildren(prev, node.path, newChildren))
    } finally {
      loadingRef.current.delete(node.path)
    }
  }, [])

  const handleActivate = useCallback((node: NodeApi<ExplorerNode>) => {
    const d = node.data
    if (d.kind === 'file' && d.ext === 'md') {
      window.dispatchEvent(
        new CustomEvent('secretariat:open-markdown', {
          detail: { path: d.path, name: d.name },
        })
      )
      return
    }
    // Directories (and other files): just toggle expansion.
    if (d.hasChildren) {
      node.toggle()
    }
  }, [])

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
      <div className="min-h-0 flex-1 overflow-hidden">
        <Tree<ExplorerNode>
          data={data}
          width={width}
          height={Math.max(height - 28, 0)}
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
      <div className="flex h-7 shrink-0 items-stretch border-t border-border bg-muted/30 text-[11px] text-muted-foreground">
        <button
          type="button"
          className="flex flex-1 items-center justify-center gap-2 px-3 transition-colors hover:bg-muted hover:text-foreground"
          onClick={refreshRoots}
          title="Refresh from filesystem"
          aria-label="Refresh from filesystem"
        >
          <RefreshCw className="h-3 w-3" />
          Refresh
        </button>
      </div>
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
        {d.hasChildren ? (
          node.isOpen ? (
            <ChevronDown className="h-3 w-3" />
          ) : (
            <ChevronRight className="h-3 w-3" />
          )
        ) : null}
      </span>
      <NodeIcon
        d={d}
        isOpen={node.isOpen}
        className="h-3.5 w-3.5 shrink-0 text-muted-foreground"
      />
      <span className="truncate">{d.name}</span>
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
  const onRename = async () => {
    const next = window.prompt(`Rename "${node.name}" to:`, node.name)
    if (next === null) return
    const trimmed = next.trim()
    if (!trimmed || trimmed === node.name) return
    const res = await commands.renamePath(node.path, trimmed)
    if (res.status === 'error') {
      toast.error(`Rename failed: ${res.error}`)
      return
    }
    toast.success(`Renamed to "${trimmed}"`)
    refreshRoots()
  }

  // Roots: reveal only (renaming would tear the vault).
  if (node.kind === 'private' || node.kind === 'org') {
    return (
      <ContextMenuItem onSelect={onReveal}>
        <FolderOpen className="h-3.5 w-3.5" />
        Reveal in Finder
      </ContextMenuItem>
    )
  }

  return (
    <>
      <ContextMenuItem onSelect={onRename}>
        <Pencil className="h-3.5 w-3.5" />
        Rename…
      </ContextMenuItem>
      <ContextMenuItem onSelect={onReveal}>
        <FolderOpen className="h-3.5 w-3.5" />
        Reveal in Finder
      </ContextMenuItem>
    </>
  )
}

function NodeIcon({
  d,
  isOpen,
  className,
}: {
  d: ExplorerNode
  isOpen: boolean
  className: string
}) {
  switch (d.kind) {
    case 'private':
      return <Lock className={className} />
    case 'org':
      return <Building2 className={className} />
    case 'channel_leaf':
      return <Hash className={className} />
    case 'dir':
      return isOpen ? (
        <FolderOpen className={className} />
      ) : (
        <Folder className={className} />
      )
    case 'file':
      return d.ext === 'md' ? (
        <FileText className={className} />
      ) : (
        <FileIcon className={className} />
      )
    default:
      return <FileIcon className={className} />
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
      return {
        ...n,
        children: spliceChildren(n.children, parentPath, children),
      }
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
