export interface MarkdownTab {
  kind: 'markdown'
  id: string
  filePath: string
  name: string
}

export type Tab = MarkdownTab

export interface PersistedTabs {
  tabs: Tab[]
  activeId: string | null
}
