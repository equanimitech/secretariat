import matter from 'gray-matter'

export type Frontmatter = Record<string, unknown>

export interface ParsedMarkdown {
  frontmatter: Frontmatter
  body: string
}

export function parseMarkdown(source: string): ParsedMarkdown {
  const parsed = matter(source)
  return {
    frontmatter: (parsed.data ?? {}) as Frontmatter,
    body: parsed.content.replace(/^\n+/, '').replace(/\n+$/, ''),
  }
}

export function serializeMarkdown(
  frontmatter: Frontmatter,
  body: string,
): string {
  if (Object.keys(frontmatter).length === 0) {
    return body
  }
  return matter.stringify(body, frontmatter)
}
