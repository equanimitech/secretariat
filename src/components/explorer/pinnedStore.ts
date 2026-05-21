/**
 * Pinned channels — Slack-style starred shortcuts at the top of the
 * sidebar. v1 is intentionally tiny: a localStorage-backed ordered set
 * of absolute channel-dir paths. Pin = surface above the tree; the
 * channel still lives in its real place below.
 *
 * Persisted under `secretariat.explorer.pinned-channels.v1` as an
 * array of absolute paths (preserves user order).
 */

const KEY = 'secretariat.explorer.pinned-channels.v1'

export interface PinnedEntry {
  /** Absolute channel-dir path. */
  path: string
  /** Channel handle (joined by `:`). */
  handle: string
  /** Last path segment / display name. */
  name: string
  /** Org alias, if any (null for `_self`). */
  org: string | null
}

type Listener = () => void

class PinnedStore {
  private entries: PinnedEntry[]
  private listeners = new Set<Listener>()

  constructor() {
    this.entries = load()
  }

  list(): PinnedEntry[] {
    return this.entries
  }

  has(path: string): boolean {
    return this.entries.some(e => e.path === path)
  }

  pin(entry: PinnedEntry) {
    if (this.has(entry.path)) return
    this.entries = [...this.entries, entry]
    save(this.entries)
    this.emit()
  }

  unpin(path: string) {
    if (!this.has(path)) return
    this.entries = this.entries.filter(e => e.path !== path)
    save(this.entries)
    this.emit()
  }

  toggle(entry: PinnedEntry) {
    if (this.has(entry.path)) {
      this.unpin(entry.path)
    } else {
      this.pin(entry)
    }
  }

  subscribe(l: Listener): () => void {
    this.listeners.add(l)
    return () => {
      this.listeners.delete(l)
    }
  }

  private emit() {
    for (const l of this.listeners) {
      l()
    }
  }
}

function load(): PinnedEntry[] {
  try {
    const raw = localStorage.getItem(KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed.filter(
      (e: unknown): e is PinnedEntry =>
        typeof e === 'object' &&
        e !== null &&
        typeof (e as PinnedEntry).path === 'string' &&
        typeof (e as PinnedEntry).handle === 'string' &&
        typeof (e as PinnedEntry).name === 'string'
    )
  } catch {
    return []
  }
}

function save(entries: PinnedEntry[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(entries))
  } catch {
    /* best effort */
  }
}

export const pinnedStore = new PinnedStore()
