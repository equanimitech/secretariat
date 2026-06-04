import { Copy } from 'lucide-react'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { useState } from 'react'
import type { LayeredVerifyResult, VerifyLayerResult } from '@/lib/bindings'
import type { TrustState } from '@/lib/markdown/trust'
import { TrustChip } from './TrustChip'

function shortDid(did: string): string {
  if (did.length <= 16) return did
  return `${did.slice(0, 12)}…${did.slice(-6)}`
}

function who(signer: string | null, selfDid: string | null): string {
  if (!signer) return 'someone'
  if (selfDid && signer === selfDid) return 'you'
  return shortDid(signer)
}

function fmtDate(iso: string | null): string | null {
  return iso ? iso.slice(0, 10) : null
}

function plainSummary(
  r: LayeredVerifyResult,
  state: TrustState,
  selfDid: string | null
): string {
  const sig = r.signature
  const stamp = r.stamp
  switch (state) {
    case 'sealed': {
      const signer = who(sig.signer ?? sig.agent, selfDid)
      const when = fmtDate(stamp.stamped_at)
      const signedClause = sig.outcome === 'none' ? '' : `Signed by ${signer}, `
      return `${signedClause}sealed${when ? ` ${when}` : ''}`
    }
    case 'signed': {
      const signer = who(sig.signer ?? sig.agent, selfDid)
      const when = fmtDate(sig.signed_at)
      return `Signed by ${signer}${when ? ` ${when}` : ''} — not yet sealed`
    }
    case 'tampered':
      return 'Fails verification — changed after signing'
    default:
      return 'Unsigned draft'
  }
}

function FineRow({ label, value }: { label: string; value: string | null }) {
  if (!value) return null
  return (
    <div className="flex gap-2 font-mono text-[11px] leading-relaxed">
      <span className="text-muted-foreground shrink-0">{label}</span>
      <span className="break-all">{value}</span>
    </div>
  )
}

function FineLayer({ name, layer }: { name: string; layer: VerifyLayerResult }) {
  return (
    <div className="space-y-0.5">
      <div className="text-muted-foreground text-[11px] font-medium uppercase tracking-wide">
        {name} · {layer.outcome}
      </div>
      <FineRow label="signer" value={layer.signer} />
      <FineRow label="agent" value={layer.agent} />
      <FineRow label="principal" value={layer.principal} />
      <FineRow label="role" value={layer.signer_role} />
      <FineRow label="signed" value={layer.signed_at} />
      <FineRow label="stamped" value={layer.stamped_at} />
      <FineRow label="act" value={layer.act} />
      <FineRow label="claimed" value={layer.claimed_hash} />
      <FineRow label="computed" value={layer.computed_hash} />
      <FineRow label="cause" value={layer.cause} />
    </div>
  )
}

/**
 * Trust home — footer, left side. Chip + plain summary; click opens the
 * provenance popover (the fine rung) upward. The single place trust state
 * is shown (AGENTS.md rule #5), no longer duplicated in the titlebar or a
 * strip above the document.
 */
export function TrustFooter({
  verify,
  state,
  selfDid,
}: {
  verify: LayeredVerifyResult | null
  state: TrustState
  selfDid: string | null
}) {
  const [copied, setCopied] = useState(false)
  const summary = verify ? plainSummary(verify, state, selfDid) : 'Verifying…'

  const onCopy = () => {
    if (!verify) return
    void navigator.clipboard.writeText(JSON.stringify(verify, null, 2))
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1500)
  }

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          title="Provenance"
          className="hover:bg-accent/50 -mx-1.5 inline-flex items-center gap-2 rounded-md px-1.5 py-0.5 transition-colors focus-visible:outline-none"
        >
          <TrustChip state={state} />
          <span className="text-muted-foreground text-xs">{summary}</span>
        </button>
      </PopoverTrigger>
      {verify && (
        <PopoverContent side="top" align="start" className="w-80 space-y-3">
          <FineLayer name="signature" layer={verify.signature} />
          <FineLayer name="stamp" layer={verify.stamp} />
          <div className="text-muted-foreground/60 text-[11px] italic">
            counter-stamp — none (reserved)
          </div>
          <button
            type="button"
            onClick={onCopy}
            className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5 text-xs"
          >
            <Copy size={12} aria-hidden />
            {copied ? 'copied' : 'copy raw JSON'}
          </button>
        </PopoverContent>
      )}
    </Popover>
  )
}
