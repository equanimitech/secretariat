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
  Pin,
  PinOff,
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
import { pinnedStore } from './pinnedStore'
import { activeChannelStore } from './activeChannel'

const SHOW_ALL_KEY = 'secretariat.explorer.show-all-files.v1'

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
  /** Unread count keyed by channel-dir path. Aggregated for parents. */
  unreadByPath: Record<string, number>
  /** Register a channel-dir path so its unread count is tracked. */
  registerPath: (path: string) => void
}

export function ExplorerTree({
  width,
  height,
  onOpenChannel,
  unreadByPath,
  registerPath,
}: ExplorerTreeProps) {
  const [data, setData] = useState<ExplorerNode[]>([])
  const [error, setError] = useState<string | null>(null)
  const [showAll, setShowAll] = useState<boolean>(() => loadShowAll())
  const [pinnedVersion, setPinnedVersion] = useState(0)
  const [activePath, setActivePath] = useState<string | null>(() =>
    activeChannelStore.get()
  )
  const loadingRef = useRef<Set<string>>(new Set())

  useEffect(() => pinnedStore.subscribe(() => setPinnedVersion(v => v + 1)), [])
  useEffect(
    () => activeChannelStore.subscribe(() => setActivePath(activeChannelStore.get())),
    []
  )

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

  // Move: drag-and-drop a channel under a new parent channel/folder.
  // We only allow within-org moves; cross-org would require handle
  // re-anchoring (deferred).
  const handleMove = useCallback(
    async ({
      dragIds,
      parentNode,
    }: {
      dragIds: string[]
      parentNode: NodeApi<ExplorerNode> | null
    }) => {
      if (!parentNode) {
        toast.error('Cannot move to the root — drop onto a channel or folder.')
        return
      }
      const dropTarget = parentNode.data
      for (const dragId of dragIds) {
        const src = findNode(data, dragId)
        if (!src) continue
        const ok = validateMove(src, dropTarget)
        if (!ok.ok) {
          toast.error(ok.error)
          continue
        }
        const res = await commands.movePath(src.path, dropTarget.path)
        if (res.status === 'error') {
          toast.error(`Move failed: ${res.error}`)
          continue
        }
        toast.success(`Moved "${src.name}" into "${dropTarget.name}"`)
      }
      refreshRoots()
    },
    [data, refreshRoots]
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
        registerPath(n.path)
      }
    })
  }, [visibleData, registerPath])

  // `pinnedVersion` is wired through the renderer context so pin/unpin
  // affecting context-menu state forces a row re-render. (Used as a
  // suppression — reference it in the renderer to silence linters.)
  void pinnedVersion

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
          onMove={handleMove}
          disableDrag={data => !canDrag(data)}
          disableDrop={({ parentNode, dragNodes }) => {
            for (const dn of dragNodes) {
              if (!validateMove(dn.data, parentNode.data).ok) return true
            }
            return false
          }}
          disableMultiSelection
        >
          {makeNodeRenderer({ refreshRoots, unreadByPath, activePath })}
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
  activePath: string | null
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
  activePath,
}: NodeRendererProps<ExplorerNode> & NodeContext) {
  const d = node.data
  const Icon = pickIcon(d, node.isOpen)
  const isActive = activePath !== null && d.path === activePath
  // Active channels always count as read — no bold, no badge.
  const rawUnread = unreadByPath[d.path] ?? 0
  const unread = isActive ? 0 : rawUnread
  const isChannelish = d.kind === 'channel_leaf' || d.hasChannelDescendants
  const bold = isChannelish && unread > 0

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
      <span className={cn('truncate', bold && 'font-semibold')}>{labelFor(d)}</span>
      {unread > 0 && <UnreadPill count={unread} />}
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

/**
 * Calm unread pill — small rounded shape, muted background, slightly
 * darker text. Deliberately not red; per leverage-points this is a
 * low-leverage feedback signal, not a notification.
 */
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

function NodeMenuItems({
  node,
  refreshRoots,
}: {
  node: ExplorerNode
  refreshRoots: () => void
}) {
  const isPinned = pinnedStore.has(node.path)
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
  const onTogglePin = () => {
    if (!node.handle) return
    pinnedStore.toggle({
      path: node.path,
      handle: node.handle,
      name: node.name,
      org: node.org,
    })
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
        <ContextMenuItem onSelect={onTogglePin}>
          {isPinned ? (
            <>
              <PinOff className="h-3.5 w-3.5" />
              Unpin
            </>
          ) : (
            <>
              <Pin className="h-3.5 w-3.5" />
              Pin
            </>
          )}
        </ContextMenuItem>
        <ContextMenuSeparator />
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

/** Channels (leaf or parent) are draggable; nothing else moves. */
function canDrag(n: ExplorerNode): boolean {
  return n.kind === 'channel_leaf'
}

interface MoveCheck {
  ok: boolean
  error: string
}

/**
 * Validate a proposed move (drag → drop). Same-org only; target must
 * be a channel or a channel-bearing folder; no cycles; no duplicate
 * basename at destination. Conservative — we lean toward false to
 * avoid silently corrupting the vault.
 */
function validateMove(src: ExplorerNode, dest: ExplorerNode): MoveCheck {
  if (src.kind !== 'channel_leaf') {
    return { ok: false, error: 'only channels can be moved' }
  }
  // Drop target must be a channel or a parent-channel folder. Org
  // and private roots aren't valid drops yet (would require handle
  // re-anchoring).
  if (dest.kind !== 'channel_leaf' && !dest.hasChannelDescendants) {
    return { ok: false, error: 'drop onto a channel or channel folder' }
  }
  // Same-org gate: refuse cross-org moves for now.
  if ((src.org ?? null) !== (dest.org ?? null)) {
    return { ok: false, error: 'cross-org moves are not supported yet' }
  }
  // Cycle guard: destination must not be src itself or a descendant of src.
  if (dest.path === src.path) {
    return { ok: false, error: 'cannot drop a channel onto itself' }
  }
  if (isDescendantPath(dest.path, src.path)) {
    return { ok: false, error: 'cannot drop a channel into its own descendant' }
  }
  // Duplicate basename guard.
  if (dest.children && dest.children.some(c => c.name === src.name)) {
    return {
      ok: false,
      error: `a "${src.name}" already exists in "${dest.name}"`,
    }
  }
  return { ok: true, error: '' }
}

/** True if `candidate` is a path under `ancestor` (or equal). */
function isDescendantPath(candidate: string, ancestor: string): boolean {
  if (candidate === ancestor) return true
  const sep = ancestor.endsWith('/') ? ancestor : `${ancestor}/`
  return candidate.startsWith(sep)
}
