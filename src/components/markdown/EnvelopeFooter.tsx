import { BadgeCheck } from 'lucide-react'
import type { LayeredVerifyResult } from '@/lib/bindings'
import type { TrustState } from '@/lib/markdown/trust'
import { TrustFooter } from './TrustFooter'

interface EnvelopeFooterProps {
  /** Coarse trust state, derived from the layered verify. */
  state: TrustState
  /** Raw layered verify result for the provenance popover. */
  verify: LayeredVerifyResult | null
  /** Current principal's DID — drives "by you" in the summary. */
  selfDid: string | null
  stamping: boolean
  saving: boolean
  onStamp: () => void
}

/**
 * Footer is the trust home: provenance on the LEFT (chip + summary +
 * popover), actions on the RIGHT (the seal now; room for 'smart' actions
 * later). Trust is shown here and nowhere else — no titlebar chip, no
 * strip above the document.
 */
export function EnvelopeFooter({
  state,
  verify,
  selfDid,
  stamping,
  saving,
  onStamp,
}: EnvelopeFooterProps) {
  // Sealing is available on a signed or unsigned doc. A sealed doc is done;
  // a tampered doc is quarantined (no seal until re-derived).
  const canSeal = state === 'signed' || state === 'unsigned'

  return (
    <footer className="border-border bg-background flex h-11 shrink-0 items-center justify-between border-t px-6">
      <TrustFooter verify={verify} state={state} selfDid={selfDid} />
      <div className="flex items-center gap-2">
        {canSeal && (
          <button
            type="button"
            onClick={onStamp}
            disabled={stamping || saving}
            className="bg-trust-sealed text-trust-sealed-fg focus-visible:ring-trust-sealed inline-flex h-8 items-center gap-1.5 rounded-md px-3.5 text-sm font-medium shadow-sm transition hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-1 active:scale-[0.98] disabled:opacity-50"
          >
            <BadgeCheck className="h-4 w-4" />
            {stamping ? 'Sealing…' : 'Seal'}
          </button>
        )}
      </div>
    </footer>
  )
}
