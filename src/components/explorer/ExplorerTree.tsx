import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
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
  Pencil,
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
import { entryToNode, isChannelTreeNode, type ExplorerNode } from './types'
import { unreadStore } from './unreadState'

const SHOW_ALL_KEY = 'secretariat.explorer.show-all-files.v1'
const ENVELOPE_OPENED_EVENT = 'secretariat:envelope-opened'

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
  const [showAll, setShowAll] = useState<boolean>(() => loadShowAll())
  const [unreadByPath, setUnreadByPath] = useState<Record<string, number>>({})
  const loadingRef = useRef<Set<string>>(new Set())
  // Paths whose unread count we've already computed; recomputed on
  // envelope-open events.
  const countedRef = useRef<Set<string>>(new Set())

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

  // Persist the show-all toggle.
  useEffect(() => {
    try {
      localStorage.setItem(SHOW_ALL_KEY, showAll ? '1' : '0')
    } catch {
      /* best effort */
    }
  }, [showAll])

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
      setData(prev =>
        spliceChildren(prev, node.path, res.data.map(e => entryToNode(e, node.org)))
      )
    } finally {
      loadingRef.current.delete(node.path)
    }
  }, [])

  // Lazy unread-count: when a channel-bearing dir appears in the
  // visible tree, walk its envelopes once and cache the count.
  const ensureUnreadCount = useCallback(async (path: string) => {
    if (countedRef.current.has(path)) return
    countedRef.current.add(path)
    const res = await commands.listEnvelopesUnder(path)
    if (res.status !== 'ok') return
    const all = res.data
    // First-touch seeding — anything not seen before counts as already
    // read (we don't want a thousand "unread" on first launch).
    for (const p of all) {
      if (!unreadStore.wasSeenPreviously(p)) {
        unreadStore.markOpened(p)
      }
    }
    const unread = all.filter(p => !unreadStore.isOpened(p)).length
    setUnreadByPath(prev => (prev[path] === unread ? prev : { ...prev, [path]: unread }))
  }, [])

  // Recompute counts for everything we've ever counted, when an
  // envelope is opened (decrement) or new envelopes arrive externally.
  const recomputeCounted = useCallback(async () => {
    const paths = [...countedRef.current]
    for (const path of paths) {
      const res = await commands.listEnvelopesUnder(path)
      if (res.status !== 'ok') continue
      const unread = res.data.filter(p => !unreadStore.isOpened(p)).length
      setUnreadByPath(prev =>
        prev[path] === unread ? prev : { ...prev, [path]: unread }
      )
    }
  }, [])

  // Listen for envelope-open events to keep ancestor counts fresh.
  useEffect(() => {
    function onOpened() {
      void recomputeCounted()
    }
    window.addEventListener(ENVELOPE_OPENED_EVENT, onOpened)
    return () => {
      window.removeEventListener(ENVELOPE_OPENED_EVENT, onOpened)
    }
  }, [recomputeCounted])

  const handleActivate = useCallback(
    (node: NodeApi<ExplorerNode>) => {
      const d = node.data
      if (d.kind === 'channel_leaf' && d.handle) {
        // Parent channels (those with channel-subdirs) behave as
        // folders only — expand/collapse, never open a tab.
        if (d.hasChannelDescendants) {
          node.toggle()
          return
        }
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
      // private / org / dir: just toggle expansion.
      if (d.hasChildren) {
        node.toggle()
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

  // Apply the channel-only filter (when showAll is false) — purely
  // a render-time projection; the underlying tree state is untouched.
  const visibleData = useMemo(
    () => (showAll ? data : filterToChannels(data)),
    [data, showAll]
  )

  // Seed unread counts for every visible channel-bearing entry.
  useEffect(() => {
    walkNodes(visibleData, n => {
      if (n.kind === 'channel_leaf' || n.hasChannelDescendants) {
        void ensureUnreadCount(n.path)
      }
    })
  }, [visibleData, ensureUnreadCount])

  return (
    <div className="flex h-full w-full flex-col bg-background">
      {error && (
        <div className="border-b border-destructive/40 bg-destructive/10 px-3 py-1.5 text-[11px] text-destructive">
          {error}
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-hidden">
        <Tree<ExplorerNode>
          data={visibleData}
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
          {makeNodeRenderer({ refreshRoots, unreadByPath })}
        </Tree>
      </div>
      <button
        type="button"
        className="flex h-7 shrink-0 items-center justify-center gap-2 border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        onClick={() => setShowAll(v => !v)}
        title={showAll ? 'Show channels only' : 'Show every file in the vault'}
      >
        {showAll ? 'Showing all files — click to hide internals' : 'Show all files'}
      </button>
    </div>
  )
}

function loadShowAll(): boolean {
  try {
    return localStorage.getItem(SHOW_ALL_KEY) === '1'
  } catch {
    return false
  }
}

interface NodeContext {
  refreshRoots: () => void
  unreadByPath: Record<string, number>
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
  unreadByPath,
}: NodeRendererProps<ExplorerNode> & NodeContext) {
  const d = node.data
  const Icon = pickIcon(d, node.isOpen)
  const unread = unreadByPath[d.path] ?? 0
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
      {unread > 0 && (
        <span
          className="ml-auto shrink-0 rounded-full bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground"
          title={`${unread} unread`}
        >
          {unread > 99 ? '99+' : unread}
        </span>
      )}
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

  // Private / org roots: no rename, no delete (would tear the vault).
  if (node.kind === 'private' || node.kind === 'org') {
    return (
      <ContextMenuItem onSelect={onReveal}>
        <FolderOpen className="h-3.5 w-3.5" />
        Reveal in Finder
      </ContextMenuItem>
    )
  }

  if (node.kind === 'channel_leaf') {
    return (
      <>
        <ContextMenuItem onSelect={onRename}>
          <Pencil className="h-3.5 w-3.5" />
          Rename…
        </ContextMenuItem>
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
  // dir
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

function labelFor(d: ExplorerNode): string {
  return d.name
}

function pickIcon(d: ExplorerNode, isOpen: boolean) {
  switch (d.kind) {
    case 'private':
      return Lock
    case 'org':
      return Building2
    case 'channel_leaf':
      // Parent channels (those with channel-subdirs) read as folders
      // in the tree — only true leaves get the channel-hash icon.
      return d.hasChannelDescendants
        ? isOpen
          ? FolderOpen
          : Folder
        : Hash
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

/**
 * Channel-only projection: keep private/org roots and every directory
 * that is (or contains) a channel. Inside `_self`/org we collapse the
 * `channels/` directory so the principal sees channels at the root
 * level instead of going through a `channels/` middleman.
 */
function filterToChannels(tree: ExplorerNode[]): ExplorerNode[] {
  return tree.map(n => projectRoot(n)).filter(Boolean) as ExplorerNode[]
}

function projectRoot(n: ExplorerNode): ExplorerNode | null {
  // Private / org roots: lift children from the `channels/` subdir
  // so they appear directly under the root. If children haven't
  // loaded yet, leave `children` undefined so the lazy-load fires
  // on first expansion.
  if (n.kind === 'private' || n.kind === 'org') {
    if (n.children === undefined) {
      return { ...n }
    }
    const channelsDir = n.children.find(
      c => c.name === 'channels' && (c.kind === 'dir' || c.kind === 'channel_leaf')
    )
    if (!channelsDir) {
      return { ...n, children: [] }
    }
    if (channelsDir.children === undefined) {
      // Children not yet loaded — surface the `channels/` dir itself
      // so the user can expand it and trigger the lazy load. The
      // node will collapse out on the next projection pass once its
      // children populate.
      return { ...n, children: [channelsDir] }
    }
    const lifted = channelsDir.children
      .map(projectInner)
      .filter(Boolean) as ExplorerNode[]
    return { ...n, children: lifted }
  }
  return projectInner(n)
}

function projectInner(n: ExplorerNode): ExplorerNode | null {
  if (!isChannelTreeNode(n)) return null
  const children =
    n.children === undefined
      ? undefined
      : (n.children.map(projectInner).filter(Boolean) as ExplorerNode[])
  return { ...n, children }
}

function walkNodes(tree: ExplorerNode[], visit: (n: ExplorerNode) => void) {
  for (const n of tree) {
    visit(n)
    if (n.children && n.children.length > 0) {
      walkNodes(n.children, visit)
    }
  }
}
