/**
 * Tiny shared store for "the currently active channel tab," used by
 * the explorer + pinned shortcuts to suppress unread badges and bold
 * styling on the channel the principal is actively viewing.
 *
 * Source of truth: `SessionTabs` dispatches updates when the active
 * tab changes. Consumers subscribe.
 */

type Listener = () => void

class ActiveChannelStore {
  private path: string | null = null
  private listeners = new Set<Listener>()

  get(): string | null {
    return this.path
  }

  set(next: string | null) {
    if (this.path === next) return
    this.path = next
    for (const l of this.listeners) l()
  }

  subscribe(l: Listener): () => void {
    this.listeners.add(l)
    return () => {
      this.listeners.delete(l)
    }
  }
}

export const activeChannelStore = new ActiveChannelStore()
