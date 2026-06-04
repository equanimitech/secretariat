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

  useEffect(() => {
    void refresh()
  }, [refresh])

  const state: TrustState = verify ? deriveTrustState(verify) : 'unsigned'
  return { verify, state, refresh, loading }
}
