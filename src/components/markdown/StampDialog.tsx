import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'

interface StampDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  body: string
  onConfirm: () => void
  loading: boolean
}

export function StampDialog({
  open,
  onOpenChange,
  body,
  onConfirm,
  loading,
}: StampDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Stamp this document</DialogTitle>
          <DialogDescription>
            Review the full body verbatim. Touch ID will gate the stamp.
          </DialogDescription>
        </DialogHeader>
        <pre className="border-border bg-muted max-h-96 overflow-auto rounded border p-3 font-mono text-xs whitespace-pre-wrap">
          {body}
        </pre>
        <DialogFooter>
          <Button
            variant="ghost"
            onClick={() => onOpenChange(false)}
            disabled={loading}
          >
            Cancel
          </Button>
          <Button onClick={onConfirm} disabled={loading}>
            {loading ? 'Stamping…' : 'Stamp'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
