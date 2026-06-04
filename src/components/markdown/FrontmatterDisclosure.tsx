import { ChevronRight } from 'lucide-react'
import type { Frontmatter } from '@/lib/markdown/parse'
import { FrontmatterPanel } from './FrontmatterPanel'

/** A lexicon `$type` like `tech.equanimi.secretariat.channelContract` reads
 * as its last segment here — the full id lives in the expanded fields. */
function shortType(type: string): string {
  const seg = type.split('.').pop()
  return seg && seg.length > 0 ? seg : type
}

function summarize(fm: Frontmatter): { label: string; detail: string | null } {
  const type = fm.type ?? fm['$type']
  const detail = typeof type === 'string' && type ? shortType(type) : null
  const agents = fm.authorized_agents
  const agentCount = Array.isArray(agents) ? agents.length : 0
  const keyCount = Object.keys(fm).length

  if (detail) {
    return {
      label: detail,
      detail: agentCount > 0 ? `${agentCount} agent${agentCount === 1 ? '' : 's'}` : null,
    }
  }
  return {
    label: 'Frontmatter',
    detail: keyCount === 0 ? 'empty' : `${keyCount} fields`,
  }
}

/**
 * Frontmatter lives ABOVE the document now (not in an offcanvas sidebar):
 * reading a document's nature is part of reading it. Collapsed → a quiet
 * one-line summary; open → the field editor inline. Native <details>.
 */
export function FrontmatterDisclosure({
  frontmatter,
  onChange,
}: {
  frontmatter: Frontmatter
  onChange: (next: Frontmatter) => void
}) {
  const { label, detail } = summarize(frontmatter)
  return (
    <details className="group">
      <summary className="text-muted-foreground hover:text-foreground flex cursor-pointer select-none items-center gap-1.5 py-1 text-sm">
        <ChevronRight
          size={13}
          aria-hidden
          className="opacity-60 transition-transform group-open:rotate-90"
        />
        <span className="font-medium">{label}</span>
        {detail && <span className="text-muted-foreground/70">· {detail}</span>}
      </summary>
      <div className="mt-2 mb-1">
        <FrontmatterPanel frontmatter={frontmatter} onChange={onChange} />
      </div>
    </details>
  )
}
