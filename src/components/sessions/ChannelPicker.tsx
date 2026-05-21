import { useEffect, useState } from 'react'
import { Plus } from 'lucide-react'
import { commands, type LaunchableChannel } from '@/lib/bindings'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'

interface ChannelPickerProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onPick: (channel: LaunchableChannel) => void
}

type Mode = { kind: 'list' } | { kind: 'create' }

export function ChannelPicker({
  open,
  onOpenChange,
  onPick,
}: ChannelPickerProps) {
  const [channels, setChannels] = useState<LaunchableChannel[] | null>(null)
  const [query, setQuery] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [mode, setMode] = useState<Mode>({ kind: 'list' })

  useEffect(() => {
    if (!open) return
    // Picker open → reset dialog state and fetch channel list via Tauri IPC.
    // No external store; the open-state transition IS the trigger.
    /* eslint-disable react-hooks/set-state-in-effect */
    setQuery('')
    setError(null)
    setMode({ kind: 'list' })
    setChannels(null)
    void commands.listLaunchableChannels().then(res => {
      if (res.status === 'ok') setChannels(res.data)
      else setError(res.error)
    })
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [open])

  const filtered = (channels ?? []).filter(c => {
    const q = query.trim().toLowerCase()
    if (!q) return true
    const hay = `${c.org ?? ''} ${c.handle} ${c.name}`.toLowerCase()
    return hay.includes(q)
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        {mode.kind === 'list' && (
          <>
            <DialogHeader>
              <DialogTitle>Open channel session</DialogTitle>
            </DialogHeader>
            <Input
              autoFocus
              placeholder="Filter channels…"
              value={query}
              onChange={e => setQuery(e.target.value)}
            />
            {error && (
              <div className="rounded border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {error}
              </div>
            )}
            <div className="max-h-72 overflow-y-auto rounded border border-border">
              {channels === null && (
                <div className="px-3 py-6 text-center text-xs text-muted-foreground">
                  Loading…
                </div>
              )}
              {channels !== null && filtered.length === 0 && (
                <div className="px-3 py-6 text-center text-xs text-muted-foreground">
                  {channels.length === 0
                    ? 'No channels yet. Create your first one below.'
                    : 'No channels match.'}
                </div>
              )}
              {filtered.map(c => (
                <button
                  key={`${c.org ?? '_self'}:${c.handle}`}
                  type="button"
                  className="flex w-full flex-col items-start gap-0.5 border-b border-border px-3 py-2 text-left text-sm last:border-b-0 hover:bg-accent"
                  onClick={() => {
                    onPick(c)
                    onOpenChange(false)
                  }}
                >
                  <span className="font-medium">
                    {c.org ? `${c.org} / ` : ''}
                    {c.name || c.handle}
                  </span>
                  <span className="font-mono text-[10px] text-muted-foreground">
                    {c.handle}
                  </span>
                </button>
              ))}
            </div>
            <Button
              variant="outline"
              className="w-full"
              onClick={() => setMode({ kind: 'create' })}
            >
              <Plus className="mr-1 h-4 w-4" />
              Create new channel
            </Button>
          </>
        )}

        {mode.kind === 'create' && (
          <CreateChannelForm
            onCancel={() => setMode({ kind: 'list' })}
            onCreated={ch => {
              onPick(ch)
              onOpenChange(false)
            }}
          />
        )}
      </DialogContent>
    </Dialog>
  )
}

function CreateChannelForm({
  onCancel,
  onCreated,
}: {
  onCancel: () => void
  onCreated: (channel: LaunchableChannel) => void
}) {
  const [handle, setHandle] = useState('')
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const submit = async () => {
    const trimmedHandle = handle.trim()
    if (!trimmedHandle) {
      setError('handle is required')
      return
    }
    setBusy(true)
    setError(null)
    const res = await commands.createChannel(
      trimmedHandle,
      name.trim() || trimmedHandle,
      description.trim(),
      null
    )
    setBusy(false)
    if (res.status === 'error') {
      setError(res.error)
      return
    }
    onCreated(res.data)
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>Create private channel</DialogTitle>
      </DialogHeader>
      <div className="flex flex-col gap-3">
        <div className="flex flex-col gap-1">
          <Label htmlFor="ch-handle">Handle</Label>
          <Input
            id="ch-handle"
            autoFocus
            value={handle}
            onChange={e => setHandle(e.target.value)}
            placeholder="myproject  or  myproject:experiment-1"
            spellCheck={false}
            className="font-mono"
          />
          <p className="text-[10px] text-muted-foreground">
            Lowercase, colon-separated segments. Stable identifier.
          </p>
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="ch-name">Display name (optional)</Label>
          <Input
            id="ch-name"
            value={name}
            onChange={e => setName(e.target.value)}
            placeholder="Defaults to the handle's last segment"
          />
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="ch-description">Description (optional)</Label>
          <Textarea
            id="ch-description"
            value={description}
            onChange={e => setDescription(e.target.value)}
            rows={2}
          />
        </div>
        {error && (
          <div className="rounded border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        )}
        <div className="flex items-center justify-between">
          <span className="text-[10px] text-muted-foreground">
            Lives in{' '}
            <span className="font-mono">~/.secretariat/_self/channels/</span>
          </span>
          <div className="flex gap-2">
            <Button variant="ghost" onClick={onCancel} disabled={busy}>
              Cancel
            </Button>
            <Button
              onClick={() => void submit()}
              disabled={busy || !handle.trim()}
            >
              {busy ? 'Creating…' : 'Create + open'}
            </Button>
          </div>
        </div>
      </div>
    </>
  )
}
