/**
 * Find-in-document for the markdown window.
 *
 * A thin adapter over `prosemirror-search`, which owns match highlighting
 * and the next/prev commands but deliberately exposes no match *count*.
 * The counter is ours, so the walk that produces it lives here — declared
 * against a structural `QueryLike` so it can be tested without standing up
 * a ProseMirror document.
 */
import type { EditorState } from '@milkdown/kit/prose/state'
import type { EditorView } from '@milkdown/kit/prose/view'
import {
  SearchQuery,
  findNext,
  findPrev,
  setSearchState,
} from 'prosemirror-search'

export interface MatchRange {
  from: number
  to: number
}

export interface MatchTally {
  /** How many times the term occurs in the document. */
  total: number
  /** 1-based position of the match the caret is sitting on, if any. */
  active: number | null
}

/** The slice of `SearchQuery` that {@link collectMatches} depends on. */
export interface QueryLike {
  valid: boolean
  findNext(state: EditorState, from?: number, to?: number): MatchRange | null
}

/**
 * Upper bound on how many matches we will enumerate for the counter. A
 * one-character term in a long document is the realistic worst case; past
 * this the exact number stops being information the reader can use.
 */
export const MATCH_CAP = 5000

export const EMPTY_TALLY: MatchTally = { total: 0, active: null }

/** Walk the whole document, collecting every hit in document order. */
export function collectMatches(
  query: QueryLike,
  state: EditorState,
  docEnd: number
): MatchRange[] {
  if (!query.valid) return []

  const found: MatchRange[] = []
  let cursor = 0

  while (found.length < MATCH_CAP && cursor <= docEnd) {
    const hit = query.findNext(state, cursor, docEnd)
    if (!hit) break
    found.push({ from: hit.from, to: hit.to })
    // A zero-width hit (`\b` and friends under a regex query) would hand
    // back the same position forever if we only advanced to `hit.to`.
    cursor = hit.to > hit.from ? hit.to : hit.from + 1
  }

  return found
}

/** Pair a match list with the caret to produce the "3 of 17" reading. */
export function tallyMatches(
  matches: MatchRange[],
  selectionFrom: number
): MatchTally {
  const index = matches.findIndex(m => m.from === selectionFrom)
  return {
    total: matches.length,
    active: index === -1 ? null : index + 1,
  }
}

/**
 * Build the query. Case-insensitive, literal — the toggles (regex, whole
 * word, case) are deliberately not in this cut.
 */
export function buildQuery(term: string): SearchQuery {
  return new SearchQuery({ search: term })
}

/** Point the editor's search plugin at `term`, highlighting its matches. */
export function applySearch(view: EditorView, term: string): void {
  view.dispatch(setSearchState(view.state.tr, buildQuery(term)))
}

/** Drop the query, clearing every highlight. */
export function clearSearch(view: EditorView): void {
  view.dispatch(setSearchState(view.state.tr, buildQuery('')))
}

/** Count the matches for `term` and locate the caret among them. */
export function readTally(view: EditorView, term: string): MatchTally {
  if (!term) return EMPTY_TALLY
  const matches = collectMatches(
    buildQuery(term),
    view.state,
    view.state.doc.content.size
  )
  return tallyMatches(matches, view.state.selection.from)
}

/** Move the caret to the next match, wrapping at the end. */
export function focusNext(view: EditorView): boolean {
  return findNext(view.state, tr => view.dispatch(tr), view)
}

/** Move the caret to the previous match, wrapping at the start. */
export function focusPrev(view: EditorView): boolean {
  return findPrev(view.state, tr => view.dispatch(tr), view)
}
