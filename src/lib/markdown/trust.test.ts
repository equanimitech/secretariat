import { describe, expect, it } from 'vitest'
import { deriveTrustState } from './trust'
import type { LayeredVerifyResult } from '../bindings'

const layer = (outcome: string) => ({
  outcome,
  signer: null,
  signer_role: null,
  principal: null,
  agent: null,
  signed_at: null,
  stamped_at: null,
  act: null,
  claimed_hash: null,
  computed_hash: null,
  cause: null,
})
const result = (sig: string, stamp: string): LayeredVerifyResult => ({
  signature: layer(sig),
  stamp: layer(stamp),
})

describe('deriveTrustState', () => {
  it('sealed when signature ok and stamp verified', () => {
    expect(deriveTrustState(result('ok', 'verified'))).toBe('sealed')
  })
  it('signed when signature ok but stamp absent', () => {
    expect(deriveTrustState(result('ok', 'none'))).toBe('signed')
  })
  it('unsigned when both absent', () => {
    expect(deriveTrustState(result('none', 'none'))).toBe('unsigned')
  })
  it('tampered when signature tampered, regardless of stamp', () => {
    expect(deriveTrustState(result('tampered', 'verified'))).toBe('tampered')
  })
  it('tampered when stamp tampered', () => {
    expect(deriveTrustState(result('ok', 'tampered'))).toBe('tampered')
  })
  it('tampered when stamp signatureInvalid (rule #5 — quarantine)', () => {
    expect(deriveTrustState(result('ok', 'signatureInvalid'))).toBe('tampered')
    expect(deriveTrustState(result('none', 'signatureInvalid'))).toBe(
      'tampered'
    )
  })
  it('signed (not unsigned) when stamp is present but signerUnresolvable', () => {
    expect(deriveTrustState(result('none', 'signerUnresolvable'))).toBe(
      'signed'
    )
  })
  it('signed (not sealed) when signer unresolvable', () => {
    expect(deriveTrustState(result('signerUnresolvable', 'none'))).toBe(
      'signed'
    )
  })
  it('sealed when only the stamp layer is present', () => {
    expect(deriveTrustState(result('none', 'verified'))).toBe('sealed')
  })
})
