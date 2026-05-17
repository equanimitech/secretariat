import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { commands } from '@/lib/tauri-bindings'
import {
  parseMarkdown,
  serializeMarkdown,
  type Frontmatter,
} from '@/lib/markdown/parse'
import { resolveTitle } from '@/lib/markdown/title'
import { CrepeEditor } from './CrepeEditor'
import { FrontmatterPanel } from './FrontmatterPanel'
import { MarkdownTitlebar } from './MarkdownTitlebar'
import { StampDialog } from './StampDialog'

interface MarkdownWindowProps {
  filePath: string
}

export function MarkdownWindow({ filePath }: MarkdownWindowProps) {
  const [frontmatter, setFrontmatter] = useState<Frontmatter>({})
  const [body, setBody] = useState('')
  const [sha256, setSha256] = useState('')
  const [saving, setSaving] = useState(false)
  const [stampOpen, setStampOpen] = useState(false)
  const [stamping, setStamping] = useState(false)
  const [loaded, setLoaded] = useState(false)
  const saveTimer = useRef<number | null>(null)

  const loadFromDisk = useCallback(async () => {
    const res = await commands.readMarkdown(filePath)
    if (res.status === 'error') {
      toast.error(`Open failed: ${res.error}`)
      return false
    }
    const parsed = parseMarkdown(res.data.content)
    setFrontmatter(parsed.frontmatter)
    setBody(parsed.body)
    setSha256(res.data.sha256)
    return true
  }, [filePath])

  useEffect(() => {
    if (!filePath) return
    void (async () => {
      const ok = await loadFromDisk()
      if (ok) setLoaded(true)
    })()
  }, [filePath, loadFromDisk])

  const title = useMemo(
    () => resolveTitle(frontmatter, body, filePath),
    [frontmatter, body, filePath],
  )

  useEffect(() => {
    document.title = title
    void getCurrentWebviewWindow()
      .setTitle(title)
      .catch(err => console.warn('setTitle failed', err))
  }, [title])

  const scheduleSave = useCallback(
    (nextFm: Frontmatter, nextBody: string) => {
      if (saveTimer.current) window.clearTimeout(saveTimer.current)
      saveTimer.current = window.setTimeout(async () => {
        setSaving(true)
        const content = serializeMarkdown(nextFm, nextBody)
        const res = await commands.writeMarkdown({
          path: filePath,
          content,
          expected_sha256: sha256,
        })
        setSaving(false)
        if (res.status === 'error') {
          toast.error(`Save failed: ${res.error}`)
          return
        }
        if (res.data.kind === 'conflict') {
          toast.error('File changed on disk — reload to merge')
          return
        }
        setSha256(res.data.sha256)
      }, 800)
    },
    [filePath, sha256],
  )

  const onStamp = useCallback(async () => {
    setStamping(true)
    const res = await commands.stampEnvelope(filePath)
    setStamping(false)
    setStampOpen(false)
    if (res.status === 'error') {
      toast.error(res.error)
      return
    }
    toast.success('Stamped')
    await loadFromDisk()
  }, [filePath, loadFromDisk])

  if (!loaded) {
    return (
      <div className="text-muted-foreground p-6 text-sm">Loading {filePath}…</div>
    )
  }

  return (
    <div className="bg-background text-foreground flex h-screen flex-col">
      <MarkdownTitlebar
        title={title}
        saving={saving}
        onStampClick={() => setStampOpen(true)}
      />
      <FrontmatterPanel
        frontmatter={frontmatter}
        onChange={next => {
          setFrontmatter(next)
          scheduleSave(next, body)
        }}
      />
      <main className="flex-1 overflow-hidden">
        <CrepeEditor
          initialValue={body}
          onChange={next => {
            setBody(next)
            scheduleSave(frontmatter, next)
          }}
        />
      </main>
      <StampDialog
        open={stampOpen}
        onOpenChange={setStampOpen}
        body={serializeMarkdown(frontmatter, body)}
        loading={stamping}
        onConfirm={onStamp}
      />
    </div>
  )
}
