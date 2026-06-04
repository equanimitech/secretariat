import { BadgeCheck } from 'lucide-react'
import { Button } from '@/components/ui/button'
import type { useVerify } from './useVerify'
import { CrepeEditor } from './CrepeEditor'
import { TrustBanner } from './TrustBanner'

const noop = () => {}

interface AttendViewProps {
  body: string
  verify: ReturnType<typeof useVerify>
  selfDid: string | null
  stamping: boolean
  onStamp: () => void
}

/**
 * The reading posture (Attend intent). Provenance comes first (TrustBanner),
 * then the body read-only with identical typography, then a bounded end
 * where the seal ceremony lives. The ceremony protocol is unchanged — hard
 * rule #4 still governs (the reader IS reading the full body verbatim here;
 * consent + Touch ID happen on click).
 */
export function AttendView({
  body,
  verify,
  selfDid,
  stamping,
  onStamp,
}: AttendViewProps) {
  const { state, verify: result } = verify
  const tampered = state === 'tampered'

  return (
    <div className="mx-auto w-full max-w-[72ch] px-6 py-8">
      <TrustBanner verify={result} state={state} selfDid={selfDid} />

      <div
        className={
          tampered
            ? 'pointer-events-none opacity-40 select-none'
            : undefined
        }
        aria-hidden={tampered}
      >
        <CrepeEditor initialValue={body} onChange={noop} readonly />
      </div>

      {/* Bounded end — a calm hairline, never an autoplay next-doc. */}
      <div className="mt-10 flex flex-col items-center gap-4">
        <div className="bg-border h-px w-16" />
        {tampered ? (
          <p className="text-trust-tampered max-w-[48ch] text-center text-sm">
            Quarantined. This document failed verification and cannot be sealed
            until its provenance is re-derived.
          </p>
        ) : state === 'sealed' ? (
          <p className="text-muted-foreground text-center text-sm">
            Sealed by you — the authoritative record.
          </p>
        ) : (
          <div className="flex flex-col items-center gap-2">
            <p className="text-muted-foreground text-center text-sm">
              {state === 'signed'
                ? 'Signed only — not yet sealed.'
                : 'Unsigned draft — not yet sealed.'}
            </p>
            <Button onClick={onStamp} disabled={stamping} className="h-8">
              <BadgeCheck size={14} className="mr-1.5" />
              {stamping ? 'Stamping…' : 'Seal this document'}
            </Button>
          </div>
        )}
      </div>
    </div>
  )
}
