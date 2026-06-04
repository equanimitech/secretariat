import { useCallback, useEffect, useState } from 'react'
import { commands, type LayeredVerifyResult } from '@/lib/bindings'
import { deriveTrustState, type TrustState } from '@/lib/markdown/trust'

export function useVerify(filePath: string) {
  const [verify, setVerify] = useState<LayeredVerifyResult | null>(null)
  const [loading, setLoading] = useState(false)

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const res = await commands.verifyEnvelope(filePath)
      // tauri-specta Result: { status: 'ok', data } | { status: 'error', error }
      if (res.status === 'ok') setVerify(res.data)
      else setVerify(null)
    } finally {
      setLoading(false)
    }
  }, [filePath])

  // Fetch on mount / filePath change. Inlined (not `refresh()`) so no
  // setState runs synchronously in the effect body — setVerify only fires
  // after the await (react-hooks/set-state-in-effect).
  useEffect(() => {
    let cancelled = false
    void (async () => {
      const res = await commands.verifyEnvelope(filePath)
      if (!cancelled) setVerify(res.status === 'ok' ? res.data : null)
    })()
    return () => {
      cancelled = true
    }
  }, [filePath])

  const state: TrustState = verify ? deriveTrustState(verify) : 'unsigned'
  return { verify, state, refresh, loading }
}
