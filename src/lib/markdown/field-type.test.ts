import { describe, it, expect } from 'vitest'
import { inferFieldType } from './field-type'

describe('inferFieldType', () => {
  it('classifies short string as text', () => {
    expect(inferFieldType('hello')).toBe('text')
  })
  it('classifies multiline string as multiline', () => {
    expect(inferFieldType('line1\nline2')).toBe('multiline')
  })
  it('classifies boolean as boolean', () => {
    expect(inferFieldType(true)).toBe('boolean')
    expect(inferFieldType(false)).toBe('boolean')
  })
  it('classifies ISO date string as date', () => {
    expect(inferFieldType('2026-05-17')).toBe('date')
    expect(inferFieldType('2026-05-17T10:30:00Z')).toBe('date')
  })
  it('classifies array of scalars as list', () => {
    expect(inferFieldType(['a', 'b'])).toBe('list')
  })
  it('classifies number as number', () => {
    expect(inferFieldType(42)).toBe('number')
  })
  it('classifies plain object as nested', () => {
    expect(inferFieldType({ k: 1 })).toBe('nested')
  })
  it('classifies null/undefined as text', () => {
    expect(inferFieldType(null)).toBe('text')
    expect(inferFieldType(undefined)).toBe('text')
  })
})
