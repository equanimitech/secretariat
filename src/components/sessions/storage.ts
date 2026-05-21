import type { ChannelTab, MarkdownTab, PersistedTabs, Tab } from './types'

const KEY = 'secretariat.session-tabs.v0'

export function loadTabs(): PersistedTabs {
  try {
    const raw = localStorage.getItem(KEY)
    if (!raw) return { tabs: [], activeId: null }
    const parsed = JSON.parse(raw) as PersistedTabs
    if (!Array.isArray(parsed.tabs)) return { tabs: [], activeId: null }
    // Backfill `kind` for pre-union storage (every saved tab was a channel).
    const tabs = parsed.tabs.map(t =>
      'kind' in t ? t : ({ ...(t as ChannelTab), kind: 'channel' } as Tab)
    )
    return { tabs, activeId: parsed.activeId ?? null }
  } catch {
    return { tabs: [], activeId: null }
  }
}

export function saveTabs(state: PersistedTabs) {
  try {
    localStorage.setItem(KEY, JSON.stringify(state))
  } catch {
    // best-effort; ignore quota errors in v0
  }
}

export function newTabId(): string {
  return `tab-${crypto.randomUUID()}`
}

export function makeChannelTab(channel: {
  handle: string
  name: string
  rootPath: string
  org: string | null
}): ChannelTab {
  return {
    kind: 'channel',
    id: newTabId(),
    channelHandle: channel.handle,
    channelName: channel.name || channel.handle,
    channelPath: channel.rootPath,
    org: channel.org,
  }
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

/** Back-compat alias for prior callers using `makeTab(channel)`. */
export const makeTab = makeChannelTab
