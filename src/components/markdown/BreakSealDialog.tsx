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

interface BreakSealDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onConfirm: () => void
  onCancel: () => void
}

/**
 * Calm interstitial raised the first time the principal edits a sealed
 * document. Editing breaks the current seal — the record reverts to
 * signed-only until re-stamped. This is care, not error: no red alarm,
 * just a clear consequence (spec Strategic Friction / construct ES-16).
 */
export function BreakSealDialog({
  open,
  onOpenChange,
  onConfirm,
  onCancel,
}: BreakSealDialogProps) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Editing breaks the current seal</AlertDialogTitle>
          <AlertDialogDescription>
            This document is sealed. Editing it reverts the record to
            signed-only until you re-stamp. The previous seal stays in its own
            commit — nothing is lost.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={onCancel}>Keep sealed</AlertDialogCancel>
          <AlertDialogAction onClick={onConfirm}>
            Edit &amp; break seal
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
