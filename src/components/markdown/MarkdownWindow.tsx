import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { Stamp } from 'lucide-react'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { Button } from '@/components/ui/button'
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

interface MarkdownWindowProps {
  filePath: string
}

type PendingSave = { frontmatter: Frontmatter; body: string }

export function MarkdownWindow({ filePath }: MarkdownWindowProps) {
  const [frontmatter, setFrontmatter] = useState<Frontmatter>({})
  const [body, setBody] = useState('')
  const [sha256, setSha256] = useState('')
  const [saving, setSaving] = useState(false)
  const [stamping, setStamping] = useState(false)
  const [loaded, setLoaded] = useState(false)
  const saveTimer = useRef<number | null>(null)
  const pendingSave = useRef<PendingSave | null>(null)
  // sha256 is captured into a ref so flushSave can await the latest value
  // without re-binding through closures on every change.
  const sha256Ref = useRef(sha256)
  useEffect(() => {
    sha256Ref.current = sha256
  }, [sha256])

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

  const performSave = useCallback(
    async (next: PendingSave): Promise<boolean> => {
      setSaving(true)
      const content = serializeMarkdown(next.frontmatter, next.body)
      const res = await commands.writeMarkdown({
        path: filePath,
        content,
        expected_sha256: sha256Ref.current,
      })
      setSaving(false)
      if (res.status === 'error') {
        toast.error(`Save failed: ${res.error}`)
        return false
      }
      if (res.data.kind === 'conflict') {
        toast.error('File changed on disk — reload to merge')
        return false
      }
      setSha256(res.data.sha256)
      sha256Ref.current = res.data.sha256
      return true
    },
    [filePath],
  )

  const scheduleSave = useCallback(
    (nextFm: Frontmatter, nextBody: string) => {
      pendingSave.current = { frontmatter: nextFm, body: nextBody }
      if (saveTimer.current) window.clearTimeout(saveTimer.current)
      saveTimer.current = window.setTimeout(async () => {
        saveTimer.current = null
        const pending = pendingSave.current
        pendingSave.current = null
        if (pending) await performSave(pending)
      }, 800)
    },
    [performSave],
  )

  const flushSave = useCallback(async (): Promise<boolean> => {
    if (saveTimer.current) {
      window.clearTimeout(saveTimer.current)
      saveTimer.current = null
    }
    const pending = pendingSave.current
    pendingSave.current = null
    if (!pending) return true
    return performSave(pending)
  }, [performSave])

  const onStamp = useCallback(async () => {
    setStamping(true)
    // The body the principal saw in the editor MUST be the body that gets
    // signed. Flush any pending debounced save first so the file on disk
    // matches what's on screen before stamp_envelope reads it.
    const flushed = await flushSave()
    if (!flushed) {
      setStamping(false)
      return
    }
    const res = await commands.stampEnvelope(filePath)
    setStamping(false)
    if (res.status === 'error') {
      toast.error(res.error)
      return
    }
    toast.success('Stamped')
    await loadFromDisk()
  }, [filePath, flushSave, loadFromDisk])

  if (!loaded) {
    return (
      <div className="text-muted-foreground p-6 text-sm">Loading {filePath}…</div>
    )
  }

  return (
    <div className="bg-background text-foreground flex h-screen flex-col">
      <MarkdownTitlebar title={title} saving={saving} />
      <div className="flex-1 overflow-y-auto">
        <FrontmatterPanel
          frontmatter={frontmatter}
          onChange={next => {
            setFrontmatter(next)
            scheduleSave(next, body)
          }}
        />
        <main>
          <CrepeEditor
            initialValue={body}
            onChange={next => {
              setBody(next)
              scheduleSave(frontmatter, next)
            }}
          />
        </main>
        <footer className="border-border flex justify-center border-t px-6 py-8">
          <Button size="lg" onClick={onStamp} disabled={stamping || saving}>
            <Stamp size={16} className="mr-2" />
            {stamping ? 'Stamping…' : 'Stamp this envelope'}
          </Button>
        </footer>
      </div>
    </div>
  )
}
