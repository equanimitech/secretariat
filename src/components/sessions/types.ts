export interface ChannelTab {
  kind: 'channel'
  id: string
  channelHandle: string
  channelPath: string
  channelName: string
  org: string | null
}

export interface MarkdownTab {
  kind: 'markdown'
  id: string
  filePath: string
  name: string
}

export type Tab = ChannelTab | MarkdownTab

/** Legacy alias for callers still importing `SessionTab`. */
export type SessionTab = ChannelTab

export interface PersistedTabs {
  tabs: Tab[]
  activeId: string | null
}
