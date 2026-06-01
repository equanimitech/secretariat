import type { TreeEntry } from '@/lib/bindings'

/** Mutable node maintained in component state — children are lazy-loaded. */
export interface ExplorerNode {
  id: string
  name: string
  path: string
  kind: TreeEntry['kind']
  hasChildren: boolean
  ext: string
  /** undefined = not loaded; [] = loaded, empty; [node, ...] = loaded with children */
  children?: ExplorerNode[]
}

export function entryToNode(e: TreeEntry): ExplorerNode {
  return {
    id: e.path,
    name: e.name,
    path: e.path,
    kind: e.kind,
    hasChildren: e.has_children,
    ext: e.ext,
    children: e.has_children ? undefined : [],
  }
}
