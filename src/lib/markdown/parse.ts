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

function loadFrontmatterObject(fmText: string): Frontmatter {
  const loaded = yaml.load(fmText)
  return loaded && typeof loaded === 'object' && !Array.isArray(loaded)
    ? (loaded as Frontmatter)
    : {}
}

// Some upstream writers (notably `sec capture --stdin` with bodies that
// themselves already carry a `---...---` block) produce files with two
// adjacent frontmatter blocks. Without this merge, the second block leaks
// into the body, gets parsed as markdown by the editor, and a roundtrip
// rewrites `_` as `\_`, `- ` as `* `, and `---` as `***` -- corrupting the
// YAML and bricking later loads.
//
// The second-block merge only fires when the body starts with `---\n`
// (adjacent blocks). A `---` after real content is a markdown HR, not a
// frontmatter block -- the old greedy loop swallowed those.
export function parseMarkdown(source: string): ParsedMarkdown {
  const m = source.match(FM_RE)
  if (!m) {
    return { frontmatter: {}, body: trim(source) }
  }
  const merged = loadFrontmatterObject(m[1] ?? '')
  let remaining = (m[2] ?? '').replace(/^\n+/, '')

  if (/^---\r?\n/.test(remaining)) {
    const m2 = remaining.match(FM_RE)
    if (m2) {
      const fm2 = loadFrontmatterObject(m2[1] ?? '')
      for (const [k, v] of Object.entries(fm2)) {
        if (!(k in merged)) merged[k] = v
      }
      remaining = (m2[2] ?? '').replace(/^\n+/, '')
    }
  }

  return { frontmatter: merged, body: trim(remaining) }
}

export function serializeMarkdown(
  frontmatter: Frontmatter,
  body: string
): string {
  if (Object.keys(frontmatter).length === 0) {
    return body
  }
  const fmText = yaml.dump(frontmatter, { lineWidth: -1 }).trimEnd()
  return `---\n${fmText}\n---\n\n${body}`
}
