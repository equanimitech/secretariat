import { useMemo } from 'react'
import { Check, Stamp } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import type { Frontmatter } from '@/lib/markdown/parse'

/**
 * Slim envelope-level action bar — always visible at the foot of the
 * envelope view so the principal never has to scroll a long body to
 * reach the stamp ceremony. Two states:
 *
 *   - Unstamped → `Stamp` button (kicks off the existing Touch-ID
 *     ceremony; presentation only, protocol unchanged).
 *   - Stamped   → `Stamped by …` pill → popover listing the stamp
 *     record (stamper, timestamp, signature prefix, doc-hash prefix).
 *
 * TODO(v0.4+ counter-stamps, AGENTS.md rule #4): the popover renders a
 * *list* of stamp records, not a single one. When counter-stamping
 * lands (m.3 process-verbaux), additional `$attestation` entries get
 * appended to `stamps` and the popover gains a second row, no shape
 * change required.
 */
export interface StampRecord {
  /** DID of the principal who attested. */
  stamper: string | null
  /** RFC 3339 timestamp from the lexicon `at` field. */
  at: string | null
  /** Detached signature (raw or prefixed `algo:<hex>`). */
  signature: string | null
  /** Document hash the signature covers (`sha256:<hex>` or bare hex). */
  docHash: string | null
}

interface EnvelopeFooterProps {
  frontmatter: Frontmatter
  /** Current principal's DID — used to render "Stamped by you". */
  selfDid: string | null
  /** Current principal's display name — falls back to "you". */
  selfDisplayName: string | null
  stamping: boolean
  saving: boolean
  onStamp: () => void
}

export function EnvelopeFooter({
  frontmatter,
  selfDid,
  selfDisplayName,
  stamping,
  saving,
  onStamp,
}: EnvelopeFooterProps) {
  const stamps = useMemo(() => collectStamps(frontmatter), [frontmatter])
  const isStamped = stamps.length > 0

  return (
    <footer className="border-border bg-background flex h-10 shrink-0 items-center justify-end gap-2 border-t px-4">
      {isStamped ? (
        <StampedPill
          stamps={stamps}
          selfDid={selfDid}
          selfDisplayName={selfDisplayName}
        />
      ) : (
        <Button
          size="sm"
          onClick={onStamp}
          disabled={stamping || saving}
          className="h-7"
        >
          <Stamp size={14} className="mr-1.5" />
          {stamping ? 'Stamping…' : 'Stamp'}
        </Button>
      )}
    </footer>
  )
}

function StampedPill({
  stamps,
  selfDid,
  selfDisplayName,
}: {
  stamps: StampRecord[]
  selfDid: string | null
  selfDisplayName: string | null
}) {
  // The "primary" stamp for the pill label is the first one in the
  // list. Counter-stamps (when they land) become additional list items
  // in the popover; the pill still summarises with the principal stamp.
  // `stamps.length > 0` is guaranteed by the caller (EnvelopeFooter
  // only renders this when `isStamped`), but we narrow defensively.
  const primary = stamps[0]
  if (!primary) return null
  const stampedBySelf = !!(
    primary.stamper &&
    selfDid &&
    primary.stamper === selfDid
  )
  const label = stampedBySelf
    ? `Stamped by ${selfDisplayName ?? 'you'}`
    : primary.stamper
      ? `Stamped by ${shortDid(primary.stamper)}`
      : 'Stamped'

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          title="Stamp details"
          className="inline-flex h-7 items-center gap-1.5 rounded-full bg-amber-100 px-3 text-xs font-medium text-amber-900 hover:bg-amber-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500 dark:bg-amber-950 dark:text-amber-200 dark:hover:bg-amber-900"
        >
          <Check className="h-3.5 w-3.5" />
          {label}
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80 p-0">
        <div className="border-border border-b px-4 py-2">
          <p className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
            Stamps
          </p>
        </div>
        <ul className="divide-y divide-border">
          {stamps.map((s, i) => (
            <li key={i} className="px-4 py-3">
              <StampDetails
                stamp={s}
                selfDid={selfDid}
                selfDisplayName={selfDisplayName}
              />
            </li>
          ))}
        </ul>
      </PopoverContent>
    </Popover>
  )
}

function StampDetails({
  stamp,
  selfDid,
  selfDisplayName,
}: {
  stamp: StampRecord
  selfDid: string | null
  selfDisplayName: string | null
}) {
  const stampedBySelf = !!(
    stamp.stamper &&
    selfDid &&
    stamp.stamper === selfDid
  )
  const stamperLabel = stampedBySelf
    ? (selfDisplayName ?? 'you')
    : stamp.stamper
      ? shortDid(stamp.stamper)
      : '—'

  return (
    <dl className="grid grid-cols-[5rem_1fr] gap-x-3 gap-y-1.5 text-xs">
      <dt className="text-muted-foreground">Stamper</dt>
      <dd className="font-mono break-all">{stamperLabel}</dd>

      <dt className="text-muted-foreground">When</dt>
      <dd>{formatTimestamp(stamp.at)}</dd>

      {stamp.signature && (
        <>
          <dt className="text-muted-foreground">Signature</dt>
          <dd className="font-mono break-all">{shortHex(stamp.signature)}</dd>
        </>
      )}

      {stamp.docHash && (
        <>
          <dt className="text-muted-foreground">Doc hash</dt>
          <dd className="font-mono break-all">{shortHex(stamp.docHash)}</dd>
        </>
      )}
    </dl>
  )
}

/**
 * Pull stamp records out of the envelope frontmatter. Today there is
 * at most one `$attestation` block per envelope (the principal's own
 * stamp). The shape is already a list to accommodate counter-stamps
 * (v0.4+, AGENTS.md rule #4) — when the lexicon gains a `counterStamps`
 * field or sibling blocks, extend this collector, the rest of the UI
 * stays put.
 */
function collectStamps(frontmatter: Frontmatter): StampRecord[] {
  const stamps: StampRecord[] = []
  const attestation = frontmatter['$attestation']
  if (
    attestation &&
    typeof attestation === 'object' &&
    !Array.isArray(attestation)
  ) {
    const record = toStampRecord(attestation as Record<string, unknown>)
    if (record) stamps.push(record)
  }
  return stamps
}

function toStampRecord(value: Record<string, unknown>): StampRecord | null {
  const stamper =
    typeof value['stamper'] === 'string'
      ? value['stamper']
      : typeof value['principal'] === 'string'
        ? value['principal']
        : null
  const at = typeof value['at'] === 'string' ? value['at'] : null
  const signature =
    typeof value['signature'] === 'string' ? value['signature'] : null
  const docHash = typeof value['docHash'] === 'string' ? value['docHash'] : null

  // We treat any object with at least one canonical field as a stamp.
  // The frontmatter walker upstream guarantees this is shaped like a
  // `tech.equanimi.secretariat.stamp` block when `$attestation` is set.
  if (!stamper && !at && !signature && !docHash) return null
  return { stamper, at, signature, docHash }
}

function shortDid(did: string): string {
  if (did.startsWith('did:key:')) {
    const tail = did.slice(8)
    return `did:key:${tail.slice(0, 8)}…`
  }
  return did.length > 32 ? `${did.slice(0, 32)}…` : did
}

function shortHex(s: string): string {
  // Accept `algo:<hex>` (e.g. `sha256:…`, `ed25519:…`) or bare hex.
  const idx = s.indexOf(':')
  if (idx >= 0) {
    return `${s.slice(0, idx + 1)}${s.slice(idx + 1, idx + 13)}…`
  }
  return s.length > 16 ? `${s.slice(0, 12)}…` : s
}

function formatTimestamp(at: string | null): string {
  if (!at) return '—'
  const d = new Date(at)
  if (Number.isNaN(d.getTime())) return at
  // Date + local time, no timezone-of-the-machine noise. Avoid using
  // `Intl.DateTimeFormat` style options that vary across locales —
  // the substrate is workshop-tool, not a date-picker.
  return d.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}
