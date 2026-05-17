import { useState } from 'react'
import { ChevronDown, ChevronRight } from 'lucide-react'
import type { Frontmatter } from '@/lib/markdown/parse'
import { FrontmatterField } from './FrontmatterField'

interface FrontmatterPanelProps {
  frontmatter: Frontmatter
  onChange: (next: Frontmatter) => void
}

export function FrontmatterPanel({
  frontmatter,
  onChange,
}: FrontmatterPanelProps) {
  const keys = Object.keys(frontmatter)
  const [open, setOpen] = useState(keys.length < 5)

  if (keys.length === 0) return null

  return (
    <div className="border-border bg-muted/30 border-b px-6 py-3">
      <button
        type="button"
        onClick={() => setOpen(o => !o)}
        className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs font-medium"
      >
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        Frontmatter
      </button>
      {open && (
        <div className="mt-2">
          {keys.map(key => (
            <FrontmatterField
              key={key}
              fieldKey={key}
              value={frontmatter[key]}
              onChange={(k, v) => onChange({ ...frontmatter, [k]: v })}
            />
          ))}
        </div>
      )}
    </div>
  )
}
