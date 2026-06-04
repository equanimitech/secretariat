import { ChevronRight } from 'lucide-react'
import type { Frontmatter } from '@/lib/markdown/parse'

function summarize(fm: Frontmatter): string {
  const parts: string[] = []
  const type = fm.type ?? fm['$type']
  if (typeof type === 'string' && type) parts.push(type)
  const agents = fm.authorized_agents
  if (Array.isArray(agents) && agents.length > 0) {
    parts.push(`${agents.length} agent${agents.length === 1 ? '' : 's'}`)
  }
  const keyCount = Object.keys(fm).length
  if (parts.length === 0) {
    parts.push(keyCount === 0 ? 'no frontmatter' : `${keyCount} fields`)
  }
  return parts.join(' · ')
}

/**
 * One-line legible summary of the document's nature, at the foot of the
 * Compose body. Frontmatter is part of reading what a document IS — no
 * longer hidden behind the offcanvas as the only access. Click expands
 * the full field editor.
 */
export function FrontmatterSummary({
  frontmatter,
  onExpand,
}: {
  frontmatter: Frontmatter
  onExpand: () => void
}) {
  return (
    <div className="mx-auto w-full max-w-[1200px] px-6 pb-8">
      <button
        type="button"
        onClick={onExpand}
        className="text-muted-foreground hover:text-foreground hover:border-border flex w-full items-center gap-1.5 rounded-md border border-transparent px-2 py-1 text-xs transition-colors"
      >
        <ChevronRight size={12} aria-hidden />
        <span className="font-mono">{summarize(frontmatter)}</span>
      </button>
    </div>
  )
}
