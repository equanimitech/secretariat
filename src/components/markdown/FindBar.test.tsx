import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi, beforeEach } from 'vitest'
import type { EditorView } from '@milkdown/kit/prose/view'
import { FindBar } from './FindBar'
import * as find from '@/lib/markdown/find'

vi.mock('@/lib/markdown/find', () => ({
  applySearch: vi.fn(),
  clearSearch: vi.fn(),
  focusNext: vi.fn(() => true),
  focusPrev: vi.fn(() => true),
  readTally: vi.fn(() => ({ total: 0, active: null })),
  EMPTY_TALLY: { total: 0, active: null },
}))

const view = {} as EditorView

function setup(overrides: Partial<Parameters<typeof FindBar>[0]> = {}) {
  const onClose = vi.fn()
  render(
    <FindBar
      open
      focusSignal={1}
      onClose={onClose}
      getView={() => view}
      {...overrides}
    />
  )
  return { onClose, user: userEvent.setup() }
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(find.readTally).mockReturnValue({ total: 0, active: null })
})

describe('FindBar', () => {
  it('renders nothing when closed', () => {
    render(
      <FindBar
        open={false}
        focusSignal={0}
        onClose={vi.fn()}
        getView={() => view}
      />
    )
    expect(screen.queryByRole('searchbox')).not.toBeInTheDocument()
  })

  it('focuses the input when opened', () => {
    setup()
    expect(screen.getByRole('searchbox')).toHaveFocus()
  })

  it('pushes the typed term into the editor search state', async () => {
    const { user } = setup()
    await user.type(screen.getByRole('searchbox'), 'seal')
    expect(find.applySearch).toHaveBeenLastCalledWith(view, 'seal')
  })

  it('shows the active position and total once there are matches', async () => {
    vi.mocked(find.readTally).mockReturnValue({ total: 17, active: 3 })
    const { user } = setup()
    await user.type(screen.getByRole('searchbox'), 'seal')
    expect(screen.getByTestId('find-count')).toHaveTextContent('3 of 17')
  })

  it('shows the total alone when the caret is not on a match', async () => {
    vi.mocked(find.readTally).mockReturnValue({ total: 17, active: null })
    const { user } = setup()
    await user.type(screen.getByRole('searchbox'), 'seal')
    expect(screen.getByTestId('find-count')).toHaveTextContent('17 matches')
  })

  it('reports no results for a term that is absent', async () => {
    vi.mocked(find.readTally).mockReturnValue({ total: 0, active: null })
    const { user } = setup()
    await user.type(screen.getByRole('searchbox'), 'zzz')
    expect(screen.getByTestId('find-count')).toHaveTextContent('No results')
  })

  it('says nothing at all while the term is empty', () => {
    setup()
    expect(screen.queryByTestId('find-count')).not.toBeInTheDocument()
  })

  it('advances on Enter', async () => {
    const { user } = setup()
    await user.type(screen.getByRole('searchbox'), 'seal{Enter}')
    expect(find.focusNext).toHaveBeenCalledWith(view)
    expect(find.focusPrev).not.toHaveBeenCalled()
  })

  it('goes back on Shift+Enter', async () => {
    const { user } = setup()
    await user.type(screen.getByRole('searchbox'), 'seal')
    await user.keyboard('{Shift>}{Enter}{/Shift}')
    expect(find.focusPrev).toHaveBeenCalledWith(view)
    expect(find.focusNext).not.toHaveBeenCalled()
  })

  it('clears the search and closes on Escape', async () => {
    const { user, onClose } = setup()
    await user.type(screen.getByRole('searchbox'), 'seal{Escape}')
    expect(find.clearSearch).toHaveBeenCalledWith(view)
    expect(onClose).toHaveBeenCalled()
  })

  it('clears the search and closes on the close button', async () => {
    const { user, onClose } = setup()
    await user.type(screen.getByRole('searchbox'), 'seal')
    await user.click(screen.getByRole('button', { name: /close find/i }))
    expect(find.clearSearch).toHaveBeenCalledWith(view)
    expect(onClose).toHaveBeenCalled()
  })

  it('does not step through matches while the term is empty', async () => {
    const { user } = setup()
    await user.type(screen.getByRole('searchbox'), '{Enter}')
    expect(find.focusNext).not.toHaveBeenCalled()
  })

  it('survives an editor view that has gone away', async () => {
    const onClose = vi.fn()
    render(
      <FindBar open focusSignal={1} onClose={onClose} getView={() => null} />
    )
    const user = userEvent.setup()
    await user.type(screen.getByRole('searchbox'), 'seal{Enter}')
    expect(find.applySearch).not.toHaveBeenCalled()
    expect(find.focusNext).not.toHaveBeenCalled()
  })
})
