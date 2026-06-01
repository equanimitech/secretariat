import type { MarkdownTab, PersistedTabs, Tab } from './types'

const KEY = 'secretariat.session-tabs.v1'

export function loadTabs(): PersistedTabs {
  try {
    const raw = localStorage.getItem(KEY)
    if (!raw) return { tabs: [], activeId: null }
    const parsed = JSON.parse(raw) as PersistedTabs
    if (!Array.isArray(parsed.tabs)) return { tabs: [], activeId: null }
    // Keep only markdown tabs — channel tabs were removed in the
    // git-native cut and any persisted ones are no longer renderable.
    const tabs = parsed.tabs.filter(
      (t): t is MarkdownTab =>
        !!t && (t as Tab).kind === 'markdown' && 'filePath' in t
    )
    const activeId =
      parsed.activeId && tabs.some(t => t.id === parsed.activeId)
        ? parsed.activeId
        : (tabs[tabs.length - 1]?.id ?? null)
    return { tabs, activeId }
  } catch {
    return { tabs: [], activeId: null }
  }
}

export function saveTabs(state: PersistedTabs) {
  try {
    localStorage.setItem(KEY, JSON.stringify(state))
  } catch {
    // best-effort; ignore quota errors
  }
}

export function newTabId(): string {
  return `tab-${crypto.randomUUID()}`
}

export function makeMarkdownTab(file: {
  path: string
  name: string
}): MarkdownTab {
  return {
    kind: 'markdown',
    id: newTabId(),
    filePath: file.path,
    name: file.name,
  }
}
