import { Button } from '@/components/ui/button'
import { Stamp } from 'lucide-react'

interface MarkdownTitlebarProps {
  title: string
  saving: boolean
  onStampClick: () => void
}

export function MarkdownTitlebar({
  title,
  saving,
  onStampClick,
}: MarkdownTitlebarProps) {
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
      <Button size="sm" onClick={onStampClick}>
        <Stamp size={14} className="mr-1.5" />
        Stamp
      </Button>
    </header>
  )
}
