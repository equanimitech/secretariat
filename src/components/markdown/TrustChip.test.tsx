import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { TrustChip } from './TrustChip'

describe('TrustChip', () => {
  it('renders the sealed label', () => {
    render(<TrustChip state="sealed" />)
    expect(screen.getByText(/sealed/i)).toBeInTheDocument()
  })
  it('renders the tampered label with an alert role', () => {
    render(<TrustChip state="tampered" />)
    expect(screen.getByRole('status')).toHaveTextContent(/tampered/i)
  })
})
