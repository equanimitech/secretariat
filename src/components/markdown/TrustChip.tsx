import { BadgeCheck, Circle, CircleDashed, TriangleAlert } from 'lucide-react'
import type { TrustState } from '@/lib/markdown/trust'

// Flat status styling — a tinted label, deliberately NOT a button. Faint
// trust-colour wash + saturated text/icon, no shadow, no border. Reads as a
// state indicator next to the (solid, raised) Seal button.
const CONFIG: Record<
  TrustState,
  { label: string; Icon: typeof BadgeCheck; cls: string }
> = {
  sealed: {
    label: 'Sealed',
    Icon: BadgeCheck,
    cls: 'bg-trust-sealed/15 text-trust-sealed',
  },
  signed: {
    label: 'Signed',
    Icon: CircleDashed,
    cls: 'bg-trust-signed/15 text-trust-signed',
  },
  unsigned: {
    label: 'Unsigned',
    Icon: Circle,
    cls: 'bg-trust-unsigned/20 text-muted-foreground',
  },
  tampered: {
    label: 'Tampered',
    Icon: TriangleAlert,
    cls: 'bg-trust-tampered/15 text-trust-tampered',
  },
}

export function TrustChip({ state }: { state: TrustState }) {
  const { label, Icon, cls } = CONFIG[state]
  return (
    <span
      role="status"
      className={`inline-flex h-6 items-center gap-1.5 rounded px-2 text-xs font-medium ${cls}`}
    >
      <Icon size={13} aria-hidden />
      {label}
    </span>
  )
}
