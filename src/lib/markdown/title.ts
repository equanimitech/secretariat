import type { Frontmatter } from './parse'

const H1 = /^#\s+(.+)$/m

export function resolveTitle(
  frontmatter: Frontmatter,
  body: string,
  filePath: string,
): string {
  const fmTitle = frontmatter.title
  if (typeof fmTitle === 'string' && fmTitle.trim().length > 0) {
    return fmTitle.trim()
  }
  const match = body.match(H1)
  if (match && match[1]) return match[1].trim()
  return basenameWithoutExt(filePath)
}

function basenameWithoutExt(p: string): string {
  const base = p.split('/').pop() ?? p
  return base.replace(/\.(md|markdown|mdown|mkd)$/i, '')
}
