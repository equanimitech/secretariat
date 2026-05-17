import { describe, it, expect } from 'vitest'
import { parseMarkdown, serializeMarkdown } from './parse'

describe('parseMarkdown', () => {
  it('parses file with frontmatter', () => {
    const src = '---\ntitle: Hello\ntags: [a, b]\n---\n# Body\n\nText.'
    const { frontmatter, body } = parseMarkdown(src)
    expect(frontmatter).toEqual({ title: 'Hello', tags: ['a', 'b'] })
    expect(body).toBe('# Body\n\nText.')
  })

  it('returns empty frontmatter when none present', () => {
    const { frontmatter, body } = parseMarkdown('# Just body')
    expect(frontmatter).toEqual({})
    expect(body).toBe('# Just body')
  })

  it('preserves unknown keys', () => {
    const src = '---\ncustom_field: value\n---\nBody'
    const { frontmatter } = parseMarkdown(src)
    expect(frontmatter).toEqual({ custom_field: 'value' })
  })
})

describe('serializeMarkdown', () => {
  it('round-trips frontmatter + body', () => {
    const src = '---\ntitle: Hello\n---\nBody.'
    const { frontmatter, body } = parseMarkdown(src)
    const out = serializeMarkdown(frontmatter, body)
    const reparsed = parseMarkdown(out)
    expect(reparsed.frontmatter).toEqual(frontmatter)
    expect(reparsed.body).toBe(body)
  })

  it('omits frontmatter delimiters when empty', () => {
    const out = serializeMarkdown({}, '# Body')
    expect(out).toBe('# Body')
  })
})
