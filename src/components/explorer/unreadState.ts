/**
 * Calm unread tracking for the explorer.
 *
 * Persists two sets in localStorage:
 *   - `opened`     — set of absolute envelope file paths the principal
 *                    has opened (in the timeline or as a markdown tab).
 *                    "Read once" — we don't re-mark on file modification.
 *   - `seenPaths`  — set of envelope paths we've ever discovered while
 *                    counting. Without this we'd count *every* envelope
 *                    as unread on first launch, which is noisy.
 *
 * Unread count for a channel-dir = |envelopes under <dir>/**\/envelopes/**\/*.md
 * minus opened|.
 *
 * Counts are computed lazily on demand by walking the directory via
 * the existing `list_dir` IPC. We cache results per channel-path in
 * memory; callers can call `invalidate(channelPath)` after opening
 * an envelope to recompute.
 */

const OPENED_KEY = 'secretariat.explorer.opened-envelopes.v1'
const SEEN_KEY = 'secretariat.explorer.seen-envelopes.v1'

type Listener = () => void

class UnreadStore {
  private opened: Set<string>
  private seen: Set<string>
  private listeners = new Set<Listener>()

  constructor() {
    this.opened = loadSet(OPENED_KEY)
    this.seen = loadSet(SEEN_KEY)
  }

  isOpened(path: string): boolean {
    return this.opened.has(path)
  }

  /** Mark a single envelope path as opened. Idempotent. */
  markOpened(path: string) {
    if (this.opened.has(path)) return
    this.opened.add(path)
    this.seen.add(path)
    saveSet(OPENED_KEY, this.opened)
    saveSet(SEEN_KEY, this.seen)
    this.emit()
  }

  /**
   * Record a batch of envelope paths discovered during a walk. New
   * paths are *not* counted as unread on first sight — they go into
   * `opened` so the explorer stays quiet until new envelopes arrive.
   * After the first seeding for a path, subsequent unseen siblings
   * count as unread.
   */
  recordSeen(paths: string[]): { newlySeen: number } {
    let added = 0
    for (const p of paths) {
      if (!this.seen.has(p)) {
        this.seen.add(p)
        added++
      }
    }
    if (added > 0) {
      saveSet(SEEN_KEY, this.seen)
    }
    return { newlySeen: added }
  }

  /** True if we've already seen this envelope on a prior walk. */
  wasSeenPreviously(path: string): boolean {
    return this.seen.has(path)
  }

  subscribe(l: Listener): () => void {
    this.listeners.add(l)
    return () => this.listeners.delete(l)
  }

  private emit() {
    for (const l of this.listeners) l()
  }
}

function loadSet(key: string): Set<string> {
  try {
    const raw = localStorage.getItem(key)
    if (!raw) return new Set()
    const arr = JSON.parse(raw) as string[]
    if (!Array.isArray(arr)) return new Set()
    return new Set(arr)
  } catch {
    return new Set()
  }
}

function saveSet(key: string, s: Set<string>) {
  try {
    localStorage.setItem(key, JSON.stringify([...s]))
  } catch {
    // best effort
  }
}

export const unreadStore = new UnreadStore()
