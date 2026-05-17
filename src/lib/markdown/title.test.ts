import { describe, it, expect } from 'vitest'
import { resolveTitle } from './title'

describe('resolveTitle', () => {
  it('prefers frontmatter.title when set', () => {
    expect(resolveTitle({ title: 'FM' }, '# Body H1', '/x/file.md')).toBe('FM')
  })
  it('falls back to first H1 in body', () => {
    expect(resolveTitle({}, '# Heading\n\nText', '/x/file.md')).toBe('Heading')
  })
  it('skips inline #-prefixed text that is not a heading', () => {
    expect(resolveTitle({}, 'intro\n# Real Heading', '/x/file.md')).toBe(
      'Real Heading',
    )
  })
  it('falls back to file basename without extension', () => {
    expect(resolveTitle({}, 'plain text', '/x/notes/my-file.md')).toBe(
      'my-file',
    )
  })
  it('returns basename when title is empty string', () => {
    expect(resolveTitle({ title: '' }, '', '/x/f.md')).toBe('f')
  })
  it('handles markdown extension variants', () => {
    expect(resolveTitle({}, '', '/x/f.markdown')).toBe('f')
    expect(resolveTitle({}, '', '/x/f.mdown')).toBe('f')
  })
})
