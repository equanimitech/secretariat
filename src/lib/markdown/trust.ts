import type { LayeredVerifyResult, VerifyLayerResult } from '../bindings'

/** Coarse trust chip state — the spec's four-state vocabulary. */
export type TrustState = 'sealed' | 'signed' | 'unsigned' | 'tampered'

const SIG_OK = new Set(['ok', 'verifiedAgent', 'okUnverifiedAgent'])
const TAMPER = new Set(['tampered', 'signatureInvalid', 'invalid'])

/**
 * Derive the coarse trust state from the layered verify result.
 * Tampered wins over everything (AGENTS.md rule #5 — a failed-signature
 * doc is quarantined). `signerUnresolvable` is "can't confirm", not
 * "tampered" — it degrades to `signed`, never up to `sealed`.
 */
export function deriveTrustState(r: LayeredVerifyResult): TrustState {
  const sig: VerifyLayerResult = r.signature
  const stamp: VerifyLayerResult = r.stamp

  if (TAMPER.has(sig.outcome) || stamp.outcome === 'tampered') return 'tampered'

  const sealed = stamp.outcome === 'verified' && (sig.outcome === 'none' || SIG_OK.has(sig.outcome))
  if (sealed) return 'sealed'

  if (SIG_OK.has(sig.outcome) || sig.outcome === 'signerUnresolvable') return 'signed'

  return 'unsigned'
}
