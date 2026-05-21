import {
  Archive,
  ArchiveRestore,
  FolderOpen,
  MoreHorizontal,
  PanelRight,
  RefreshCw,
} from 'lucide-react'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useSidebar } from '@/components/ui/sidebar'
import { commands } from '@/lib/bindings'
import { classifyEnvelopePath } from '@/lib/envelope-path'

interface MarkdownTitlebarProps {
  title: string
  saving: boolean
  filePath: string
  onReload: () => void
  reloading: boolean
}

// NOTE: "Launch Claude" used to live in this toolbar. It is a
// channel-level action (it `cd`s into the channel root, not the
// envelope file), so it now lives in the channel header — see
// `ChannelTimeline.tsx`.
export function MarkdownTitlebar({
  title,
  saving,
  filePath,
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

  const { isEnvelope, isArchived } = classifyEnvelopePath(filePath)
  const onArchive = async () => {
    const res = await commands.archiveInboxEnvelope(filePath)
    if (res.status === 'error') {
      toast.error(`Archive failed: ${res.error}`)
      return
    }
    toast.success('Archived')
  }
  const onUnarchive = async () => {
    const res = await commands.unarchiveInboxEnvelope(filePath)
    if (res.status === 'error') {
      toast.error(`Unarchive failed: ${res.error}`)
      return
    }
    toast.success('Unarchived')
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
        {(isEnvelope || isArchived) && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm" title="More actions">
                <MoreHorizontal size={14} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {isArchived ? (
                <DropdownMenuItem onSelect={onUnarchive}>
                  <ArchiveRestore className="h-3.5 w-3.5" />
                  Unarchive
                </DropdownMenuItem>
              ) : (
                <DropdownMenuItem onSelect={onArchive}>
                  <Archive className="h-3.5 w-3.5" />
                  Archive
                </DropdownMenuItem>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </div>
    </header>
  )
}
