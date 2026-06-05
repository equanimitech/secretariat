import { useState } from 'react'
import { toast } from 'sonner'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { compose, send } from '@/lib/dispatch/dispatch-client'
import type { ComposeResult } from '@/lib/tauri-bindings'

interface DispatchComposerProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Absolute path of the document being dispatched. */
  docPath: string
}

type Phase = 'instruct' | 'composing' | 'review' | 'sending'

export function DispatchComposer({ open, onOpenChange, docPath }: DispatchComposerProps) {
  const [phase, setPhase] = useState<Phase>('instruct')
  const [instruction, setInstruction] = useState('')
  const [draft, setDraft] = useState<ComposeResult | null>(null)

  function reset() {
    setPhase('instruct')
    setInstruction('')
    setDraft(null)
  }

  async function handleCompose() {
    if (!instruction.trim()) return
    setPhase('composing')
    try {
      const result = await compose(docPath, instruction.trim())
      setDraft(result)
      setPhase('review')
    } catch (e) {
      toast.error(`Compose failed: ${e instanceof Error ? e.message : String(e)}`)
      setPhase('instruct')
    }
  }

  async function handleSend() {
    if (!draft) return
    setPhase('sending')
    try {
      const result = await send(draft.channel, draft.body)
      toast.success(
        `Sent to ${draft.channel}`,
        result.permalink ? { description: result.permalink } : undefined,
      )
      onOpenChange(false)
      reset()
    } catch (e) {
      toast.error(`Send failed: ${e instanceof Error ? e.message : String(e)}`)
      setPhase('review')
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        onOpenChange(o)
        if (!o) reset()
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Send to Slack</DialogTitle>
          <DialogDescription>
            The scribe drafts the message; you review the exact text before it sends.
            Sends the saved document.
          </DialogDescription>
        </DialogHeader>

        {phase !== 'review' && phase !== 'sending' ? (
          <Textarea
            placeholder="e.g. send a short summary to #legal"
            value={instruction}
            onChange={(e) => setInstruction(e.target.value)}
            disabled={phase === 'composing'}
            rows={3}
          />
        ) : (
          <div className="space-y-2">
            <div className="text-sm text-muted-foreground">
              Channel: <span className="font-mono">{draft?.channel}</span>
            </div>
            <Textarea readOnly value={draft?.body ?? ''} rows={8} className="font-mono text-sm" />
          </div>
        )}

        <DialogFooter>
          {phase === 'review' || phase === 'sending' ? (
            <>
              <Button
                variant="outline"
                onClick={() => setPhase('instruct')}
                disabled={phase === 'sending'}
              >
                Back
              </Button>
              <Button onClick={handleSend} disabled={phase === 'sending'}>
                {phase === 'sending' ? 'Sending…' : 'Send'}
              </Button>
            </>
          ) : (
            <Button
              onClick={handleCompose}
              disabled={phase === 'composing' || !instruction.trim()}
            >
              {phase === 'composing' ? 'Composing…' : 'Compose'}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
