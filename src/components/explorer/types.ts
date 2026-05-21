import type { TreeEntry } from '@/lib/bindings'

/** Mutable node maintained in component state — children are lazy-loaded. */
export interface ExplorerNode {
  id: string
  name: string
  path: string
  kind: TreeEntry['kind']
  hasChildren: boolean
  /** True if this dir (or any descendant) contains a `channel.md`. */
  hasChannelDescendants: boolean
  ext: string
  handle: string | null
  org: string | null
  /** undefined = not loaded; [] = loaded, empty; [node, ...] = loaded with children */
  children?: ExplorerNode[]
}

export function entryToNode(
  e: TreeEntry,
  parentOrg: string | null = null
): ExplorerNode {
  return {
    id: e.path,
    name: e.name,
    path: e.path,
    kind: e.kind,
    hasChildren: e.has_children,
    hasChannelDescendants: e.has_channel_descendants,
    ext: e.ext,
    handle: e.handle,
    org: e.org ?? parentOrg,
    children: e.has_children ? undefined : [],
  }
}

/** Names of substrate dirs that never carry channels — hidden in channel-only mode. */
const NON_CHANNEL_DIR_NAMES = new Set([
  'envelopes',
  '_drafts',
  'sent',
  '_ciphertext',
  '.claude',
  'identity',
  'lexicon',
  'peers',
  'queues',
  'bin',
  'logs',
  '.archive',
])

/** Decide whether to display a node in channel-only mode. */
export function isChannelTreeNode(n: ExplorerNode): boolean {
  if (n.kind === 'private' || n.kind === 'org') return true
  if (n.kind === 'file') return false
  // dir | channel_leaf: only if it contains channels somewhere.
  if (NON_CHANNEL_DIR_NAMES.has(n.name)) return false
  if (n.kind === 'channel_leaf') return true
  return n.hasChannelDescendants
}
