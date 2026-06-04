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

  // Tamper wins on EITHER layer. TAMPER is the single source of truth for
  // what counts as an integrity failure — `signatureInvalid` is the stamp
  // layer's equivalent of the signature layer's `invalid`, so an invalid
  // stamp is quarantined too (AGENTS.md rule #5), never shown as signed/unsigned.
  if (TAMPER.has(sig.outcome) || TAMPER.has(stamp.outcome)) return 'tampered'

  const sealed =
    stamp.outcome === 'verified' &&
    (sig.outcome === 'none' || SIG_OK.has(sig.outcome))
  if (sealed) return 'sealed'

  // "Can't confirm" on either layer degrades to `signed` (informational) —
  // never up to `sealed`, never down to `unsigned` (a present-but-unconfirmable
  // stamp still carries provenance; don't offer it as a fresh draft to seal).
  if (
    SIG_OK.has(sig.outcome) ||
    sig.outcome === 'signerUnresolvable' ||
    stamp.outcome === 'signerUnresolvable'
  )
    return 'signed'

  return 'unsigned'
}
