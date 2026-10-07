import { describe, it, expect, vi, beforeAll } from 'vitest'
import { createRef } from 'react'
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { WorklogTab } from './WorklogTab'
import type { Worklog } from '@/entities/worklog/model'

beforeAll(() => {
  i18n.changeLanguage('en')
})

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

const worklogs: Worklog[] = [
  {
    id: 'w1',
    issueId: 'i1',
    userId: 'u1',
    userDisplayName: 'Alice',
    timeSpentSeconds: 3600,
    startedAt: '2026-08-01T10:00:00Z',
    comment: 'Fixed the bug',
    createdAt: '2026-08-01T10:00:00Z',
    updatedAt: '2026-08-01T10:00:00Z',
  },
  {
    id: 'w2',
    issueId: 'i1',
    userId: 'u2',
    userDisplayName: 'Bob',
    timeSpentSeconds: 1800,
    startedAt: '2026-08-02T10:00:00Z',
    comment: null,
    createdAt: '2026-08-02T10:00:00Z',
    updatedAt: '2026-08-02T10:00:00Z',
  },
]

describe('WorklogTab', () => {
  it('renders worklog entries', () => {
    render(
      wrapper(
        <WorklogTab worklogs={worklogs} onEdit={vi.fn()} onDelete={vi.fn()} currentUserId="u1" />,
      ),
    )
    // Names appear in both desktop table and mobile card
    expect(screen.getAllByText('Alice').length).toBeGreaterThanOrEqual(1)
    expect(screen.getAllByText('Bob').length).toBeGreaterThanOrEqual(1)
    expect(screen.getAllByText('Fixed the bug').length).toBeGreaterThanOrEqual(1)
  })

  it('renders total logged time', () => {
    render(
      wrapper(
        <WorklogTab worklogs={worklogs} onEdit={vi.fn()} onDelete={vi.fn()} currentUserId="u1" />,
      ),
    )
    expect(screen.getByText(/total logged/i)).toBeInTheDocument()
    // 3600 + 1800 = 5400s = 1h 30m
    expect(screen.getByText('1h 30m')).toBeInTheDocument()
  })

  it('renders empty state when no worklogs', () => {
    render(
      wrapper(<WorklogTab worklogs={[]} onEdit={vi.fn()} onDelete={vi.fn()} currentUserId="u1" />),
    )
    expect(screen.getByText(/no entries yet/i)).toBeInTheDocument()
  })

  it('shows edit button only for current user worklogs', () => {
    render(
      wrapper(
        <WorklogTab worklogs={worklogs} onEdit={vi.fn()} onDelete={vi.fn()} currentUserId="u1" />,
      ),
    )
    // Alice's worklog (u1 === currentUserId) should have edit buttons (desktop + mobile)
    expect(screen.getAllByLabelText(/edit worklog/i)).toHaveLength(2)
  })

  it('keeps delete confirmation open when the request fails', async () => {
    const onDelete = vi.fn().mockRejectedValue(new Error('Server rejected deletion'))
    render(
      wrapper(
        <WorklogTab worklogs={worklogs} onEdit={vi.fn()} onDelete={onDelete} currentUserId="u1" />,
      ),
    )
    fireEvent.click(screen.getAllByLabelText(/delete worklog/i)[0]!)
    fireEvent.click(screen.getByRole('button', { name: /confirm/i }))
    await waitFor(() => expect(onDelete).toHaveBeenCalledWith('w1'))
    expect(await screen.findByRole('alert')).toHaveTextContent('Server rejected deletion')
    expect(screen.getByRole('alertdialog')).toBeInTheDocument()
  })

  it('clears a failed deletion before confirming a different worklog', async () => {
    const owned = worklogs.map((worklog) => ({ ...worklog, userId: 'u1' }))
    render(
      wrapper(
        <WorklogTab
          worklogs={owned}
          onEdit={vi.fn()}
          onDelete={vi.fn().mockRejectedValue(new Error('Previous deletion failed'))}
          currentUserId="u1"
        />,
      ),
    )
    fireEvent.click(screen.getAllByLabelText(/delete worklog/i)[0]!)
    fireEvent.click(screen.getByRole('button', { name: /confirm/i }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Previous deletion failed')
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    fireEvent.click(screen.getAllByLabelText(/delete worklog/i)[1]!)
    expect(screen.getByRole('alertdialog')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it.each([false, true])(
    'keeps pending confirmation mounted and restores focus after empty=%s',
    async (empty) => {
      let complete!: () => void
      const onDelete = vi.fn(
        () =>
          new Promise<void>((resolve) => {
            complete = resolve
          }),
      )
      const fallbackFocusRef = createRef<HTMLButtonElement>()
      const content = (entries: Worklog[]) =>
        wrapper(
          <>
            <button ref={fallbackFocusRef}>Worklog tab</button>
            <WorklogTab
              worklogs={entries}
              onEdit={vi.fn()}
              onDelete={onDelete}
              currentUserId="u1"
              fallbackFocusRef={fallbackFocusRef}
            />
          </>,
        )
      const page = render(content(worklogs))
      const trigger = screen.getAllByLabelText(/delete worklog/i)[0]!
      trigger.focus()
      fireEvent.click(trigger)
      fireEvent.click(screen.getByRole('button', { name: /confirm/i }))
      page.rerender(content(empty ? [] : [worklogs[1]!]))
      expect(screen.getByRole('alertdialog')).toBeInTheDocument()
      expect(screen.getByRole('button', { name: /confirm/i })).toBeDisabled()
      await act(async () => complete())
      await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
      await waitFor(() => expect(fallbackFocusRef.current).toHaveFocus())
    },
  )
})
