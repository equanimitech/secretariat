import { useState } from 'react'
import { Copy } from 'lucide-react'
import type { LayeredVerifyResult, VerifyLayerResult } from '@/lib/bindings'
import type { TrustState } from '@/lib/markdown/trust'
import { TrustChip } from './TrustChip'

/** Short DID for display — `did:key:z6Mk…last6`. Never the full string in
 * the medium rung (that's the fine rung's job). */
function shortDid(did: string): string {
  if (did.length <= 16) return did
  return `${did.slice(0, 12)}…${did.slice(-6)}`
}

function who(layerSigner: string | null, selfDid: string | null): string {
  if (!layerSigner) return 'someone'
  if (selfDid && layerSigner === selfDid) return 'you'
  return shortDid(layerSigner)
}

function fmtDate(iso: string | null): string | null {
  if (!iso) return null
  // Date-only, locale-stable: the day is what a reader cares about.
  return iso.slice(0, 10)
}

/** Plain-language medium rung — NOT lexicon jargon (spec acceptance gate). */
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
      return `${signedClause}sealed${when ? ` ${when}` : ''}.`
    }
    case 'signed': {
      const signer = who(sig.signer ?? sig.agent, selfDid)
      const when = fmtDate(sig.signed_at)
      return `Signed by ${signer}${when ? ` ${when}` : ''} — not yet sealed.`
    }
    case 'tampered':
      return 'This document fails verification — its contents changed after signing.'
    default:
      return 'Unsigned draft — no author signature yet.'
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

interface TrustBannerProps {
  verify: LayeredVerifyResult | null
  state: TrustState
  selfDid: string | null
}

/**
 * Provenance-forward banner. Three rungs of zoom (AGENTS.md rule #5 —
 * the reader derives trust from this, not from the body alone):
 *   coarse  — the TrustChip + a one-line plain-language summary
 *   medium  — [details] discloses who/when in plain words
 *   fine    — raw layer fields, the reserved counter row, copy-JSON
 */
export function TrustBanner({ verify, state, selfDid }: TrustBannerProps) {
  const [copied, setCopied] = useState(false)
  const summary = verify ? plainSummary(verify, state, selfDid) : 'Verifying…'

  const onCopy = () => {
    if (!verify) return
    void navigator.clipboard.writeText(JSON.stringify(verify, null, 2))
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1500)
  }

  return (
    <div className="border-border bg-background/60 mx-auto mb-6 w-full max-w-[68ch] rounded-lg border px-4 py-3">
      <div className="flex items-center gap-3">
        <TrustChip state={state} />
        <span className="text-muted-foreground text-sm">{summary}</span>
      </div>
      {verify && (
        <details className="mt-2 group">
          <summary className="text-muted-foreground hover:text-foreground cursor-pointer select-none text-xs">
            details
          </summary>
          <div className="mt-3 space-y-3">
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
          </div>
        </details>
      )}
    </div>
  )
}
