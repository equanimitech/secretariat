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
  const initialValueRef = useRef(initialValue)

  useEffect(() => {
    onChangeRef.current = onChange
  })

  useEffect(() => {
    if (!hostRef.current) return
    const crepe = new Crepe({
      root: hostRef.current,
      defaultValue: initialValueRef.current,
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
  }, [])

  return <div ref={hostRef} className="prose-host h-full overflow-auto" />
}
