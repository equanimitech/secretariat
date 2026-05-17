export type FieldType =
  | 'text'
  | 'multiline'
  | 'boolean'
  | 'date'
  | 'list'
  | 'number'
  | 'nested'

const ISO_DATE =
  /^\d{4}-\d{2}-\d{2}(T\d{2}:\d{2}(:\d{2})?(\.\d+)?(Z|[+-]\d{2}:?\d{2})?)?$/

export function inferFieldType(value: unknown): FieldType {
  if (value === null || value === undefined) return 'text'
  if (typeof value === 'boolean') return 'boolean'
  if (typeof value === 'number') return 'number'
  if (Array.isArray(value)) return 'list'
  if (typeof value === 'object') return 'nested'
  if (typeof value === 'string') {
    if (value.includes('\n')) return 'multiline'
    if (ISO_DATE.test(value)) return 'date'
    return 'text'
  }
  return 'text'
}
