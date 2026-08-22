import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import type { EditorView } from '@milkdown/kit/prose/view'
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
import { FindBar } from './FindBar'
import { MarkdownTitlebar } from './MarkdownTitlebar'
import { BreakSealDialog } from './BreakSealDialog'
import { FrontmatterDisclosure } from './FrontmatterDisclosure'
import { useVerify } from './useVerify'
import './markdown-editor.css'

interface MarkdownWindowProps {
  filePath: string
  /** When true, layout fills the parent (`h-full`) instead of `h-screen`
   * and we do not mutate the webview window's title. The MarkdownTitlebar
   * still renders — its reload/reveal/sidebar/archive actions are useful
   * inside a tab, even though the tab strip already names the document. */
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
  // Find-in-document. The view arrives from CrepeEditor and is re-published
  // on every remount (reload, seal state flip), so FindBar reads it lazily
  // through a ref rather than capturing it.
  const editorViewRef = useRef<EditorView | null>(null)
  const [findOpen, setFindOpen] = useState(false)
  const [findFocusSignal, setFindFocusSignal] = useState(0)
  const verify = useVerify(filePath)
  const [breakSealOpen, setBreakSealOpen] = useState(false)
  const [sealBroken, setSealBroken] = useState(false)
  const pendingBreakValue = useRef<string | null>(null)
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
    // A freshly loaded (or re-stamped) doc re-arms the break-seal prompt.
    setSealBroken(false)
    return true
  }, [filePath])

  useEffect(() => {
    if (!filePath) return
    void (async () => {
      const ok = await loadFromDisk()
      if (ok) setLoaded(true)
    })()
  }, [filePath, loadFromDisk])

  // The principal's DID drives "by you" in the trust summary. Static for
  // the window's lifetime — load once.
  useEffect(() => {
    void (async () => {
      const ident = await commands.currentIdentity()
      if (ident.status === 'ok' && ident.data) setSelfDid(ident.data.did)
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
      if (!(e.metaKey || e.ctrlKey) || e.shiftKey || e.altKey) return
      if (e.key === 'r') {
        e.preventDefault()
        requestReload()
        return
      }
      if (e.key === 'f') {
        e.preventDefault()
        setFindOpen(true)
        // Bumped unconditionally: a second Cmd+F on an already-open bar
        // should re-focus and select the term, not fall through silently.
        setFindFocusSignal(n => n + 1)
      }
    }
    window.addEventListener('keydown', handler, { capture: true })
    return () =>
      window.removeEventListener('keydown', handler, { capture: true })
  }, [requestReload])

  // Body change. If the doc is sealed and the seal hasn't been broken this
  // session, intercept the first edit and raise the calm interstitial rather
  // than silently invalidating the seal.
  const onBodyChange = useCallback(
    (next: string) => {
      if (verify.state === 'sealed' && !sealBroken) {
        pendingBreakValue.current = next
        setBreakSealOpen(true)
        return
      }
      setBody(next)
      scheduleSave(frontmatter, next)
    },
    [verify.state, sealBroken, frontmatter, scheduleSave]
  )

  const onConfirmBreakSeal = useCallback(() => {
    setBreakSealOpen(false)
    setSealBroken(true)
    const next = pendingBreakValue.current
    pendingBreakValue.current = null
    if (next !== null) {
      setBody(next)
      scheduleSave(frontmatter, next)
    }
  }, [frontmatter, scheduleSave])

  const onCancelBreakSeal = useCallback(() => {
    setBreakSealOpen(false)
    pendingBreakValue.current = null
    // Revert the editor to the last committed body by remounting Crepe.
    setEditorKey(k => k + 1)
  }, [])

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
    await verify.refresh()
  }, [filePath, flushSave, loadFromDisk, verify])

  if (!loaded) {
    return (
      <div className="text-muted-foreground p-6 text-sm">
        Loading {filePath}…
      </div>
    )
  }

  return (
    <div
      className={cn(
        'bg-background text-foreground flex min-h-0 flex-col',
        embedded ? 'h-full' : 'h-screen'
      )}
    >
      <MarkdownTitlebar
        title={title}
        saving={saving}
        filePath={filePath}
        onReload={requestReload}
        reloading={reloading}
      />
      <FindBar
        open={findOpen}
        focusSignal={findFocusSignal}
        onClose={() => setFindOpen(false)}
        getView={() => editorViewRef.current}
      />
      <div className="flex-1 overflow-y-auto">
        <div className="editor-shell py-8">
          {/* Document header — frontmatter preview above the body. Trust
              now lives in the footer, not here. */}
          <FrontmatterDisclosure
            frontmatter={frontmatter}
            onChange={next => {
              setFrontmatter(next)
              scheduleSave(next, body)
            }}
          />
          <CrepeEditor
            key={`${editorKey}-${verify.state === 'sealed' ? 'ro' : 'rw'}`}
            initialValue={body}
            onChange={onBodyChange}
            readonly={verify.state === 'sealed'}
            onViewReady={view => {
              editorViewRef.current = view
            }}
          />
        </div>
      </div>
      <EnvelopeFooter
        state={verify.state}
        verify={verify.verify}
        selfDid={selfDid}
        stamping={stamping}
        saving={saving}
        onStamp={onStamp}
      />
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
      <BreakSealDialog
        open={breakSealOpen}
        onOpenChange={setBreakSealOpen}
        onConfirm={onConfirmBreakSeal}
        onCancel={onCancelBreakSeal}
      />
    </div>
  )
}
