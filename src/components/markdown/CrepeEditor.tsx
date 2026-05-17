import { useEffect, useRef } from 'react'
import { Crepe } from '@milkdown/crepe'
import '@milkdown/crepe/theme/common/style.css'
import '@milkdown/crepe/theme/frame.css'

interface CrepeEditorProps {
  initialValue: string
  onChange: (markdown: string) => void
}

export function CrepeEditor({ initialValue, onChange }: CrepeEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const onChangeRef = useRef(onChange)
  onChangeRef.current = onChange

  useEffect(() => {
    if (!hostRef.current) return
    const crepe = new Crepe({
      root: hostRef.current,
      defaultValue: initialValue,
    })
    crepe.on(api => {
      api.markdownUpdated((_ctx, markdown) => {
        onChangeRef.current(markdown)
      })
    })
    void crepe.create()
    return () => {
      void crepe.destroy()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return <div ref={hostRef} className="prose-host h-full overflow-auto" />
}
