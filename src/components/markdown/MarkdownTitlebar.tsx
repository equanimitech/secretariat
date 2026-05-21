import { FolderOpen, PanelRight } from 'lucide-react'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { Button } from '@/components/ui/button'
import { useSidebar } from '@/components/ui/sidebar'

interface MarkdownTitlebarProps {
  title: string
  saving: boolean
  filePath: string
}

// NOTE: "Launch Claude" used to live in this toolbar. It is a
// channel-level action (it `cd`s into the channel root, not the
// envelope file), so it now lives in the channel header — see
// `ChannelTimeline.tsx`.
export function MarkdownTitlebar({
  title,
  saving,
  filePath,
}: MarkdownTitlebarProps) {
  const { toggleSidebar } = useSidebar()

  const onReveal = async () => {
    try {
      await revealItemInDir(filePath)
    } catch (err) {
      console.warn('revealItemInDir failed', err)
    }
  }

  return (
    <header
      data-tauri-drag-region
      className="border-border bg-background flex items-center justify-between border-b px-6 py-2"
    >
      <div className="flex items-center gap-2" data-tauri-drag-region>
        <h1
          className="text-base font-semibold tracking-tight"
          data-tauri-drag-region
        >
          {title}
        </h1>
        {saving && (
          <span className="text-muted-foreground text-xs">Saving…</span>
        )}
      </div>
      <div className="flex items-center gap-1">
        <Button
          variant="ghost"
          size="sm"
          onClick={onReveal}
          title="Reveal in Finder"
        >
          <FolderOpen size={14} />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={toggleSidebar}
          title="Frontmatter"
        >
          <PanelRight size={14} />
        </Button>
      </div>
    </header>
  )
}
