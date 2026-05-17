import type { Frontmatter } from '@/lib/markdown/parse'
import { FrontmatterField } from './FrontmatterField'

interface FrontmatterPanelProps {
  frontmatter: Frontmatter
  onChange: (next: Frontmatter) => void
}

/**
 * Field list for the right sidebar. The Sidebar primitive owns the
 * open/closed state and chrome — this just renders the editable fields.
 */
export function FrontmatterPanel({
  frontmatter,
  onChange,
}: FrontmatterPanelProps) {
  const keys = Object.keys(frontmatter)
  if (keys.length === 0) {
    return (
      <p className="text-muted-foreground px-3 py-4 text-xs">
        No frontmatter on this envelope.
      </p>
    )
  }
  return (
    <div className="space-y-2 px-3 py-3">
      {keys.map(key => (
        <FrontmatterField
          key={key}
          fieldKey={key}
          value={frontmatter[key]}
          onChange={(k, v) => onChange({ ...frontmatter, [k]: v })}
        />
      ))}
    </div>
  )
}
