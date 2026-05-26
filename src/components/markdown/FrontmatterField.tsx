import { useState } from 'react'
import { ChevronDown, ChevronRight, Lock } from 'lucide-react'
import { inferFieldType, type FieldType } from '@/lib/markdown/field-type'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Switch } from '@/components/ui/switch'

interface FrontmatterFieldProps {
  fieldKey: string
  value: unknown
  onChange: (key: string, newValue: unknown) => void
}

export function FrontmatterField({
  fieldKey,
  value,
  onChange,
}: FrontmatterFieldProps) {
  // Protocol-metadata blocks ($envelope, $attestation, ...) are
  // surfaced as a single collapsed labeled row with the salient facts.
  // They are read-only — the principal does not hand-edit protocol
  // records; the lexicon shape is authoritative.
  if (fieldKey.startsWith('$') && isObject(value)) {
    return <ProtocolBlock fieldKey={fieldKey} value={value} />
  }

  const type = inferFieldType(value)
  return (
    <div className="flex items-start gap-3 py-1.5">
      <label className="text-muted-foreground w-32 shrink-0 pt-1.5 text-sm">
        {fieldKey}
      </label>
      <div className="flex-1">
        {renderControl(type, value, v => onChange(fieldKey, v))}
      </div>
    </div>
  )
}

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
}

/**
 * Collapsed-by-default view of a `$`-prefixed frontmatter block (e.g.
 * `$envelope`, `$attestation`). Shows the `$type` and a few key facts;
 * expand reveals the full pretty-printed JSON, read-only.
 */
function ProtocolBlock({
  fieldKey,
  value,
}: {
  fieldKey: string
  value: Record<string, unknown>
}) {
  const [open, setOpen] = useState(false)
  const facts = summarizeFacts(fieldKey, value)
  const typeId =
    typeof value['$type'] === 'string' ? (value['$type'] as string) : null

  return (
    <div className="rounded-md border border-border/60 bg-muted/30">
      <button
        type="button"
        onClick={() => setOpen(o => !o)}
        className="flex w-full items-start gap-2 px-2 py-2 text-left hover:bg-muted/50"
        aria-expanded={open}
      >
        <span className="mt-0.5 text-muted-foreground">
          {open ? (
            <ChevronDown className="h-3.5 w-3.5" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5" />
          )}
        </span>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <Lock className="h-3 w-3 text-muted-foreground" aria-hidden />
            <span className="font-mono text-xs font-medium text-foreground">
              {fieldKey}
            </span>
            {typeId && (
              <span className="truncate font-mono text-[10px] text-muted-foreground">
                {shortenType(typeId)}
              </span>
            )}
          </div>
          {facts.length > 0 && (
            <dl className="mt-1 grid grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 text-[11px]">
              {facts.map(f => (
                <div key={f.key} className="contents">
                  <dt className="text-muted-foreground">{f.label}</dt>
                  <dd className="truncate font-mono text-foreground/80">
                    {f.value}
                  </dd>
                </div>
              ))}
            </dl>
          )}
        </div>
      </button>
      {open && (
        <pre className="overflow-x-auto border-t border-border/60 bg-background/60 px-2 py-2 font-mono text-[11px] leading-relaxed text-foreground/80">
          {JSON.stringify(value, null, 2)}
        </pre>
      )}
    </div>
  )
}

interface Fact {
  key: string
  label: string
  value: string
}

/**
 * Pick the key facts to surface for a protocol block based on its
 * `$type`. Falls back to a generic "first scalar fields" projection so
 * unknown block kinds still render something useful.
 */
function summarizeFacts(
  fieldKey: string,
  value: Record<string, unknown>
): Fact[] {
  const typeId = typeof value['$type'] === 'string' ? value['$type'] : null

  // tech.equanimi.secretariat.envelope — surface from/to/handle/at when
  // available. `at` is filename-derived elsewhere, but envelopes carry
  // `cadenceHint` etc.; we lean on the canonical fields.
  if (
    fieldKey === '$envelope' ||
    typeId === 'tech.equanimi.secretariat.envelope'
  ) {
    const facts: Fact[] = []
    if (typeof value['from'] === 'string')
      facts.push({ key: 'from', label: 'from', value: shortDid(value['from']) })
    if (typeof value['to'] === 'string')
      facts.push({ key: 'to', label: 'to', value: shortDid(value['to']) })
    if (typeof value['handle'] === 'string')
      facts.push({ key: 'handle', label: 'handle', value: value['handle'] })
    return facts
  }

  // tech.equanimi.secretariat.stamp — surface stamper + signature.
  if (
    fieldKey === '$attestation' ||
    typeId === 'tech.equanimi.secretariat.stamp'
  ) {
    const facts: Fact[] = []
    const stamper = value['stamper'] ?? value['principal']
    if (typeof stamper === 'string')
      facts.push({
        key: 'stamper',
        label: 'stamper',
        value: shortDid(stamper),
      })
    if (typeof value['at'] === 'string')
      facts.push({ key: 'at', label: 'at', value: value['at'] })
    if (typeof value['signature'] === 'string')
      facts.push({
        key: 'sig',
        label: 'sig',
        value: shortHex(value['signature']),
      })
    if (typeof value['docHash'] === 'string')
      facts.push({
        key: 'docHash',
        label: 'docHash',
        value: shortHex(value['docHash']),
      })
    return facts
  }

  // Generic fallback — pick the first few scalar fields.
  const facts: Fact[] = []
  for (const [k, v] of Object.entries(value)) {
    if (k === '$type') continue
    if (
      typeof v === 'string' ||
      typeof v === 'number' ||
      typeof v === 'boolean'
    ) {
      facts.push({ key: k, label: k, value: String(v) })
    }
    if (facts.length >= 4) break
  }
  return facts
}

function shortDid(did: string): string {
  if (did.startsWith('did:key:')) {
    const tail = did.slice(8)
    return `did:key:${tail.slice(0, 8)}…`
  }
  return did.length > 40 ? `${did.slice(0, 40)}…` : did
}

function shortHex(s: string): string {
  // Accept `sha256:<hex>` or bare hex; show a short prefix either way.
  const idx = s.indexOf(':')
  if (idx >= 0) {
    return `${s.slice(0, idx + 1)}${s.slice(idx + 1, idx + 13)}…`
  }
  return s.length > 16 ? `${s.slice(0, 12)}…` : s
}

function shortenType(t: string): string {
  // tech.equanimi.secretariat.envelope → envelope
  const parts = t.split('.')
  return parts[parts.length - 1] ?? t
}

function renderControl(
  type: FieldType,
  value: unknown,
  set: (v: unknown) => void
) {
  switch (type) {
    case 'boolean':
      return <Switch checked={Boolean(value)} onCheckedChange={set} />
    case 'multiline':
      return (
        <Textarea
          value={String(value ?? '')}
          onChange={e => set(e.target.value)}
          rows={4}
        />
      )
    case 'date':
      return (
        <Input
          type="date"
          value={String(value ?? '').slice(0, 10)}
          onChange={e => set(e.target.value)}
        />
      )
    case 'number':
      return (
        <Input
          type="number"
          value={Number(value ?? 0)}
          onChange={e => set(Number(e.target.value))}
        />
      )
    case 'list':
      return (
        <Input
          value={(Array.isArray(value) ? value : []).map(String).join(', ')}
          onChange={e =>
            set(
              e.target.value
                .split(',')
                .map(s => s.trim())
                .filter(Boolean)
            )
          }
          placeholder="comma, separated, list"
        />
      )
    case 'nested':
      return (
        <Textarea
          readOnly
          value={JSON.stringify(value, null, 2)}
          rows={4}
          className="font-mono text-xs"
        />
      )
    case 'text':
    default:
      return (
        <Input
          value={String(value ?? '')}
          onChange={e => set(e.target.value)}
        />
      )
  }
}
