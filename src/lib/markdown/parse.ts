import yaml from 'js-yaml'

export type Frontmatter = Record<string, unknown>

export interface ParsedMarkdown {
  frontmatter: Frontmatter
  body: string
}

const FM_RE = /^---\r?\n([\s\S]+?)\r?\n---\r?\n?([\s\S]*)$/

function trim(s: string): string {
  return s.replace(/^\n+/, '').replace(/\n+$/, '')
}

export function parseMarkdown(source: string): ParsedMarkdown {
  const match = source.match(FM_RE)
  if (!match) {
    return { frontmatter: {}, body: trim(source) }
  }
  const fmText = match[1] ?? ''
  const body = match[2] ?? ''
  const loaded = yaml.load(fmText)
  const frontmatter =
    loaded && typeof loaded === 'object' && !Array.isArray(loaded)
      ? (loaded as Frontmatter)
      : {}
  return { frontmatter, body: trim(body) }
}

export function serializeMarkdown(
  frontmatter: Frontmatter,
  body: string,
): string {
  if (Object.keys(frontmatter).length === 0) {
    return body
  }
  const fmText = yaml.dump(frontmatter, { lineWidth: -1 }).trimEnd()
  return `---\n${fmText}\n---\n\n${body}`
}
