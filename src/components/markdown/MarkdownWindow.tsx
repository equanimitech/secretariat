import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { buttonVariants } from '@/components/ui/button'
import {
  Sidebar,
  SidebarContent,
  SidebarHeader,
  SidebarInset,
  SidebarProvider,
} from '@/components/ui/sidebar'
import { cn } from '@/lib/utils'
import { commands } from '@/lib/tauri-bindings'
import {
  parseMarkdown,
  serializeMarkdown,
  type Frontmatter,
} from '@/lib/markdown/parse'
import { resolveTitle } from '@/lib/markdown/title'
import { CrepeEditor } from './CrepeEditor'
import { EnvelopeFooter } from './EnvelopeFooter'
import { FrontmatterPanel } from './FrontmatterPanel'
import { MarkdownTitlebar } from './MarkdownTitlebar'

interface MarkdownWindowProps {
  filePath: string
  /** When true, layout fills the parent (`h-full`) instead of `h-screen`,
   * the titlebar is omitted (the host's tab strip names the document),
   * and we do not mutate the webview window's title. */
  embedded?: boolean
}

interface PendingSave {
  frontmatter: Frontmatter
  body: string
}

export function MarkdownWindow({
  filePath,
  embedded = false,
}: MarkdownWindowProps) {
  const [frontmatter, setFrontmatter] = useState<Frontmatter>({})
  const [body, setBody] = useState('')
  const [sha256, setSha256] = useState('')
  const [saving, setSaving] = useState(false)
  const [stamping, setStamping] = useState(false)
  const [loaded, setLoaded] = useState(false)
  // CrepeEditor consumes `initialValue` only at mount. Bumping this key
  // remounts it with the freshly-loaded body when we reload from disk.
  const [editorKey, setEditorKey] = useState(0)
  const [selfDid, setSelfDid] = useState<string | null>(null)
  const [selfDisplayName, setSelfDisplayName] = useState<string | null>(null)
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

  // Identity + profile feed the "Stamped by <you|name>" pill label.
  // Both are static for the lifetime of the window — load once.
  useEffect(() => {
    void (async () => {
      const [ident, prof] = await Promise.all([
        commands.currentIdentity(),
        commands.getProfile(),
      ])
      if (ident.status === 'ok' && ident.data) setSelfDid(ident.data.did)
      if (prof.status === 'ok' && prof.data)
        setSelfDisplayName(prof.data.display_name)
    })()
  }, [])

  const title = useMemo(
    () => resolveTitle(frontmatter, body, filePath),
    [frontmatter, body, filePath]
  )

  useEffect(() => {
    if (embedded) return
    document.title = title
    void getCurrentWebviewWindow()
      .setTitle(title)
      .catch(err => console.warn('setTitle failed', err))
  }, [title, embedded])

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
    [filePath]
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
    [performSave]
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

  // Reload-from-disk. VS Code semantics: silent when clean, prompt
  // (Save / Discard / Cancel) when there are pending unsaved edits the
  // debounced autosave hasn't flushed yet.
  const [reloadDialogOpen, setReloadDialogOpen] = useState(false)
  const [reloading, setReloading] = useState(false)

  const hasUnsavedEdits = useCallback(
    () => pendingSave.current !== null || saveTimer.current !== null,
    []
  )

  const doReload = useCallback(async () => {
    setReloading(true)
    const ok = await loadFromDisk()
    setReloading(false)
    if (!ok) return
    setEditorKey(k => k + 1)
    toast.success('Reloaded from disk')
  }, [loadFromDisk])

  const requestReload = useCallback(() => {
    if (hasUnsavedEdits()) {
      setReloadDialogOpen(true)
      return
    }
    void doReload()
  }, [hasUnsavedEdits, doReload])

  const onSaveAndReload = useCallback(async () => {
    setReloadDialogOpen(false)
    const flushed = await flushSave()
    if (!flushed) return
    await doReload()
  }, [flushSave, doReload])

  const onDiscardAndReload = useCallback(async () => {
    setReloadDialogOpen(false)
    if (saveTimer.current) {
      window.clearTimeout(saveTimer.current)
      saveTimer.current = null
    }
    pendingSave.current = null
    await doReload()
  }, [doReload])

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (
        (e.metaKey || e.ctrlKey) &&
        !e.shiftKey &&
        !e.altKey &&
        e.key === 'r'
      ) {
        e.preventDefault()
        requestReload()
      }
    }
    window.addEventListener('keydown', handler, { capture: true })
    return () =>
      window.removeEventListener('keydown', handler, { capture: true })
  }, [requestReload])

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
      <div className="text-muted-foreground p-6 text-sm">
        Loading {filePath}…
      </div>
    )
  }

  return (
    <SidebarProvider
      defaultOpen={false}
      className={cn('min-h-0', embedded ? 'h-full' : 'h-screen')}
    >
      <SidebarInset
        className={cn(
          'bg-background text-foreground flex min-h-0 flex-col',
          embedded ? 'h-full' : 'h-screen'
        )}
      >
        {!embedded && (
          <MarkdownTitlebar
            title={title}
            saving={saving}
            filePath={filePath}
            onReload={requestReload}
            reloading={reloading}
          />
        )}
        <div className="flex-1 overflow-y-auto">
          <main>
            <CrepeEditor
              key={editorKey}
              initialValue={body}
              onChange={next => {
                setBody(next)
                scheduleSave(frontmatter, next)
              }}
            />
          </main>
        </div>
        <EnvelopeFooter
          frontmatter={frontmatter}
          selfDid={selfDid}
          selfDisplayName={selfDisplayName}
          stamping={stamping}
          saving={saving}
          onStamp={onStamp}
        />
      </SidebarInset>
      <Sidebar side="right" collapsible="offcanvas">
        <SidebarHeader>
          <h2 className="text-muted-foreground px-3 py-2 text-xs font-medium uppercase tracking-wider">
            Frontmatter
          </h2>
        </SidebarHeader>
        <SidebarContent>
          <FrontmatterPanel
            frontmatter={frontmatter}
            onChange={next => {
              setFrontmatter(next)
              scheduleSave(next, body)
            }}
          />
        </SidebarContent>
      </Sidebar>
      <AlertDialog open={reloadDialogOpen} onOpenChange={setReloadDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Reload from disk?</AlertDialogTitle>
            <AlertDialogDescription>
              This file has unsaved edits that haven&apos;t been written yet.
              Discarding will replace the editor contents with the version on
              disk.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              className={buttonVariants({ variant: 'destructive' })}
              onClick={onDiscardAndReload}
            >
              Discard &amp; reload
            </AlertDialogAction>
            <AlertDialogAction onClick={onSaveAndReload}>
              Save &amp; reload
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SidebarProvider>
  )
}
