import {
  Eye,
  FileCode,
  FolderOpen,
  PanelRight,
  Pencil,
  RefreshCw,
} from 'lucide-react'
import { openPath, revealItemInDir } from '@tauri-apps/plugin-opener'
import { Button } from '@/components/ui/button'
import { useSidebar } from '@/components/ui/sidebar'
import { cn } from '@/lib/utils'
import type { TrustState } from '@/lib/markdown/trust'
import { TrustChip } from './TrustChip'

type Intent = 'compose' | 'attend'

interface MarkdownTitlebarProps {
  title: string
  saving: boolean
  filePath: string
  intent: Intent
  onToggleIntent: () => void
  trust: TrustState
  onReload: () => void
  reloading: boolean
}

export function MarkdownTitlebar({
  title,
  saving,
  filePath,
  intent,
  onToggleIntent,
  trust,
  onReload,
  reloading,
}: MarkdownTitlebarProps) {
  const { toggleSidebar } = useSidebar()

  const onReveal = async () => {
    try {
      await revealItemInDir(filePath)
    } catch (err) {
      console.warn('revealItemInDir failed', err)
    }
  }

  const onEditRaw = async () => {
    try {
      await openPath(filePath)
    } catch (err) {
      console.warn('openPath failed', err)
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
        <span
          title={saving ? 'Saving…' : 'Synced — on disk'}
          aria-label={saving ? 'Saving' : 'Synced'}
          className={cn(
            'ml-1 inline-block size-2 rounded-full transition-colors',
            saving ? 'bg-muted-foreground animate-pulse' : 'bg-trust-sealed'
          )}
        />
        {saving && (
          <span className="text-muted-foreground/0 sr-only">Saving</span>
        )}
      </div>
      <div className="flex items-center gap-1">
        <TrustChip state={trust} />
        <Button
          variant="ghost"
          size="sm"
          onClick={onToggleIntent}
          title={
            intent === 'compose'
              ? 'Attend — read & seal (⌘E)'
              : 'Compose — edit (⌘E)'
          }
          aria-label="Toggle Compose and Attend"
          className="gap-1.5"
        >
          {intent === 'compose' ? (
            <>
              <Eye size={14} /> Attend
            </>
          ) : (
            <>
              <Pencil size={14} /> Compose
            </>
          )}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={onReload}
          disabled={reloading}
          title="Reload from disk (⌘R)"
          aria-label="Reload from disk"
        >
          <RefreshCw
            size={14}
            className={reloading ? 'animate-spin' : undefined}
          />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={onEditRaw}
          title="Edit raw .md in your default editor"
          aria-label="Edit raw markdown"
        >
          <FileCode size={14} />
        </Button>
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
