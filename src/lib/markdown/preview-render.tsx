/**
 * Minimal markdown renderer for timeline previews.
 *
 * Scope: just enough to make the body slice in `EnvelopePreview.preview`
 * legible in a 3-line card — heading lines, bullet/numbered list markers,
 * bold, italics, inline code. No tables, no links, no images, no code
 * blocks (the preview is line-truncated upstream so block constructs
 * cannot safely render).
 *
 * Why not a library: the timeline preview is ~3 lines of plaintext-ish
 * markdown; full milkdown/marked footprint is not justified, and Crepe
 * is editor-shaped, not list-renderer-shaped. If we ever need the full
 * grammar, swap this for `marked` behind the same interface.
 */

import { Fragment, type ReactNode } from 'react'

export interface RenderOptions {
  /** Max lines to render. Lines past this are dropped. Default 3. */
  maxLines?: number
}

/**
 * Render a short markdown string as a flow of React nodes. Each source
 * line maps to either a block (heading / list-item / paragraph line);
 * the inline tokens are resolved per-line.
 */
export function renderPreviewMarkdown(
  source: string,
  options: RenderOptions = {},
): ReactNode {
  const maxLines = options.maxLines ?? 3
  const lines = source.split('\n').slice(0, maxLines)
  return (
    <>
      {lines.map((line, i) => (
        <Fragment key={i}>{renderLine(line, i)}</Fragment>
      ))}
    </>
  )
}

function renderLine(line: string, lineIdx: number): ReactNode {
  const heading = line.match(/^(#{1,6})\s+(.*)$/)
  if (heading) {
    const level = (heading[1] ?? '').length
    const text = heading[2] ?? ''
    return (
      <div
        className={
          'font-semibold text-foreground ' +
          (level <= 2 ? 'text-sm' : 'text-[13px]')
        }
      >
        {renderInline(text)}
      </div>
    )
  }

  const ul = line.match(/^\s*[-*+]\s+(.*)$/)
  if (ul) {
    return (
      <div className="flex gap-1.5">
        <span className="select-none text-muted-foreground">•</span>
        <span className="flex-1">{renderInline(ul[1] ?? '')}</span>
      </div>
    )
  }

  const ol = line.match(/^\s*(\d+)\.\s+(.*)$/)
  if (ol) {
    return (
      <div className="flex gap-1.5">
        <span className="select-none text-muted-foreground">{ol[1]}.</span>
        <span className="flex-1">{renderInline(ol[2] ?? '')}</span>
      </div>
    )
  }

  const bq = line.match(/^>\s?(.*)$/)
  if (bq) {
    return (
      <div className="border-l-2 border-border pl-2 italic text-foreground/80">
        {renderInline(bq[1] ?? '')}
      </div>
    )
  }

  if (line.trim() === '') {
    return lineIdx === 0 ? null : <div className="h-1" />
  }

  return <div>{renderInline(line)}</div>
}

interface Token {
  kind: 'text' | 'bold' | 'italic' | 'code'
  value: string
}

/**
 * Tokenize a single line by repeatedly applying `String.prototype.match`
 * to find the next inline marker. Returns segments in order.
 */
function tokenizeInline(text: string): Token[] {
  const tokens: Token[] = []
  let rest = text
  const pattern = /(\*\*([^*]+)\*\*)|(\*([^*]+)\*)|(_([^_]+)_)|(`([^`]+)`)/
  while (rest.length > 0) {
    const m = rest.match(pattern)
    if (!m || m.index === undefined) {
      tokens.push({ kind: 'text', value: rest })
      break
    }
    if (m.index > 0) {
      tokens.push({ kind: 'text', value: rest.slice(0, m.index) })
    }
    if (m[2] !== undefined) tokens.push({ kind: 'bold', value: m[2] })
    else if (m[4] !== undefined) tokens.push({ kind: 'italic', value: m[4] })
    else if (m[6] !== undefined) tokens.push({ kind: 'italic', value: m[6] })
    else if (m[8] !== undefined) tokens.push({ kind: 'code', value: m[8] })
    rest = rest.slice(m.index + m[0].length)
  }
  return tokens
}

function renderInline(text: string): ReactNode {
  const tokens = tokenizeInline(text)
  if (tokens.length === 0) return text
  return tokens.map((t, i) => {
    switch (t.kind) {
      case 'bold':
        return <strong key={i}>{t.value}</strong>
      case 'italic':
        return <em key={i}>{t.value}</em>
      case 'code':
        return (
          <code
            key={i}
            className="rounded bg-muted px-1 py-0.5 font-mono text-[11px]"
          >
            {t.value}
          </code>
        )
      case 'text':
      default:
        return <Fragment key={i}>{t.value}</Fragment>
    }
  })
}
