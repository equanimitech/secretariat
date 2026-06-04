import { BadgeCheck, Circle, CircleDashed, TriangleAlert } from 'lucide-react'
import type { TrustState } from '@/lib/markdown/trust'

const CONFIG: Record<TrustState, { label: string; Icon: typeof BadgeCheck; cls: string }> = {
  sealed: { label: 'Sealed', Icon: BadgeCheck, cls: 'bg-trust-sealed text-trust-sealed-fg' },
  signed: { label: 'Signed', Icon: CircleDashed, cls: 'bg-trust-signed text-trust-signed-fg' },
  unsigned: { label: 'Unsigned', Icon: Circle, cls: 'bg-trust-unsigned text-trust-unsigned-fg' },
  tampered: {
    label: 'Tampered',
    Icon: TriangleAlert,
    cls: 'bg-trust-tampered text-trust-tampered-fg',
  },
}

export function TrustChip({ state }: { state: TrustState }) {
  const { label, Icon, cls } = CONFIG[state]
  return (
    <span
      role="status"
      className={`inline-flex h-7 items-center gap-1.5 rounded-md px-2.5 text-xs font-medium ${cls}`}
    >
      <Icon size={14} aria-hidden />
      {label}
    </span>
  )
}
