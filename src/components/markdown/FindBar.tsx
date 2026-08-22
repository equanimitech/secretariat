import { useCallback, useEffect, useRef, useState } from 'react'
import type { EditorView } from '@milkdown/kit/prose/view'
import { ChevronDown, ChevronUp, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  applySearch,
  clearSearch,
  focusNext,
  focusPrev,
  readTally,
  EMPTY_TALLY,
  type MatchTally,
} from '@/lib/markdown/find'

interface FindBarProps {
  open: boolean
  /**
   * Bumped by the owner every time Cmd+F fires, so a second press while the
   * bar is already open re-focuses and selects the term rather than doing
   * nothing.
   */
  focusSignal: number
  onClose: () => void
  /** Resolved lazily — the editor view outlives neither remounts nor reloads. */
  getView: () => EditorView | null
}

function describe(tally: MatchTally): string {
  if (tally.total === 0) return 'No results'
  if (tally.active === null) {
    return tally.total === 1 ? '1 match' : `${tally.total} matches`
  }
  return `${tally.active} of ${tally.total}`
}

/**
 * Find-in-document. Crepe ships no search feature, so this drives the
 * `prosemirror-search` plugin registered in {@link CrepeEditor} directly.
 *
 * Find only — no replace. A sealed document is read-only and the seal is
 * broken by editing, so a bulk-rewrite affordance sitting one keystroke from
 * the reader is the wrong first move here.
 */
export function FindBar({ open, focusSignal, onClose, getView }: FindBarProps) {
  const [term, setTerm] = useState('')
  const [tally, setTally] = useState<MatchTally>(EMPTY_TALLY)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (!open) return
    const input = inputRef.current
    if (!input) return
    input.focus()
    input.select()
  }, [open, focusSignal])

  const retally = useCallback((view: EditorView, next: string) => {
    setTally(next ? readTally(view, next) : EMPTY_TALLY)
  }, [])

  const onTermChange = useCallback(
    (next: string) => {
      setTerm(next)
      const view = getView()
      if (!view) return
      applySearch(view, next)
      retally(view, next)
    },
    [getView, retally]
  )

  const step = useCallback(
    (direction: 'next' | 'prev') => {
      if (!term) return
      const view = getView()
      if (!view) return
      const moved = direction === 'next' ? focusNext(view) : focusPrev(view)
      if (moved) retally(view, term)
    },
    [term, getView, retally]
  )

  const close = useCallback(() => {
    const view = getView()
    if (view) clearSearch(view)
    setTerm('')
    setTally(EMPTY_TALLY)
    onClose()
  }, [getView, onClose])

  if (!open) return null

  return (
    <div className="bg-background flex shrink-0 items-center gap-2 border-b px-3 py-2">
      <Input
        ref={inputRef}
        type="search"
        role="searchbox"
        aria-label="Find in document"
        placeholder="Find"
        value={term}
        className="h-8 max-w-xs"
        onChange={e => onTermChange(e.target.value)}
        onKeyDown={e => {
          if (e.key === 'Enter') {
            e.preventDefault()
            step(e.shiftKey ? 'prev' : 'next')
            return
          }
          if (e.key === 'Escape') {
            e.preventDefault()
            close()
          }
        }}
      />

      {term && (
        <span
          data-testid="find-count"
          aria-live="polite"
          className="text-muted-foreground min-w-24 text-xs tabular-nums"
        >
          {describe(tally)}
        </span>
      )}

      <div className="ml-auto flex items-center gap-1">
        <Button
          variant="ghost"
          size="icon"
          className="size-7"
          aria-label="Previous match"
          disabled={!term || tally.total === 0}
          onClick={() => step('prev')}
        >
          <ChevronUp className="size-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="size-7"
          aria-label="Next match"
          disabled={!term || tally.total === 0}
          onClick={() => step('next')}
        >
          <ChevronDown className="size-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="size-7"
          aria-label="Close find"
          onClick={close}
        >
          <X className="size-4" />
        </Button>
      </div>
    </div>
  )
}
