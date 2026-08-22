import { describe, it, expect, vi } from 'vitest'
import type { EditorState } from '@milkdown/kit/prose/state'
import {
  collectMatches,
  tallyMatches,
  MATCH_CAP,
  type MatchRange,
  type QueryLike,
} from './find'

/** A stub standing in for SearchQuery over a flat list of known hits. */
function stubQuery(hits: MatchRange[], valid = true): QueryLike {
  return {
    valid,
    findNext: (_state, from = 0, to = Number.MAX_SAFE_INTEGER) =>
      hits.find(h => h.from >= from && h.to <= to) ?? null,
  }
}

const state = {} as EditorState

describe('collectMatches', () => {
  it('returns every hit in document order', () => {
    const hits = [
      { from: 3, to: 8 },
      { from: 20, to: 25 },
      { from: 41, to: 46 },
    ]
    expect(collectMatches(stubQuery(hits), state, 100)).toEqual(hits)
  })

  it('returns nothing for an invalid query', () => {
    const q = stubQuery([{ from: 1, to: 2 }], false)
    expect(collectMatches(q, state, 100)).toEqual([])
  })

  it('returns nothing when the document has no hits', () => {
    expect(collectMatches(stubQuery([]), state, 100)).toEqual([])
  })

  it('terminates on zero-width matches instead of spinning', () => {
    // A regex like `\b` matches without consuming. Advancing by `to` alone
    // would return the same position forever.
    const query: QueryLike = {
      valid: true,
      findNext: (_state, from = 0) => (from < 4 ? { from, to: from } : null),
    }
    const found = collectMatches(query, state, 100)
    expect(found).toEqual([
      { from: 0, to: 0 },
      { from: 1, to: 1 },
      { from: 2, to: 2 },
      { from: 3, to: 3 },
    ])
  })

  it('stops at MATCH_CAP on a pathological document', () => {
    const query: QueryLike = {
      valid: true,
      findNext: (_state, from = 0) => ({ from, to: from + 1 }),
    }
    expect(collectMatches(query, state, Number.MAX_SAFE_INTEGER)).toHaveLength(
      MATCH_CAP
    )
  })

  it('does not search past the end of the document', () => {
    const findNext = vi.fn(() => null)
    collectMatches({ valid: true, findNext }, state, 10)
    expect(findNext).toHaveBeenCalledWith(state, 0, 10)
  })
})

describe('tallyMatches', () => {
  const hits = [
    { from: 3, to: 8 },
    { from: 20, to: 25 },
    { from: 41, to: 46 },
  ]

  it('counts the total', () => {
    expect(tallyMatches(hits, 3).total).toBe(3)
  })

  it('reports a 1-based active index when the selection sits on a match', () => {
    expect(tallyMatches(hits, 20).active).toBe(2)
  })

  it('reports no active match when the selection is elsewhere', () => {
    expect(tallyMatches(hits, 15).active).toBeNull()
  })

  it('reports zero and no active index for an empty result set', () => {
    expect(tallyMatches([], 0)).toEqual({ total: 0, active: null })
  })
})
