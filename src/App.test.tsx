import { render } from '@/test/test-utils'
import { describe, it, expect } from 'vitest'
import App from './App'

// Tauri bindings are mocked globally in src/test/setup.ts

describe('App', () => {
  it('renders main window layout', () => {
    const { container } = render(<App />)
    expect(container.firstChild).toBeInTheDocument()
  })
})
