import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@/shared/lib/theme'
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
})
