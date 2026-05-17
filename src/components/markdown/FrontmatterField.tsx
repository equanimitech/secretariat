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

function renderControl(
  type: FieldType,
  value: unknown,
  set: (v: unknown) => void,
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
                .filter(Boolean),
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
