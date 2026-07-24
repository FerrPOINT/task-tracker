import { describe, it, expect, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter, Routes, Route } from 'react-router'

import { ProjectBoardPage } from './'
import { ThemeProvider } from '@/shared/lib/theme'

const getBoard = vi.hoisted(() =>
  vi.fn(() =>
    Promise.resolve({
      project_key: 'TT',
      sprint: { id: 's1', name: 'Sprint 1', remaining_days: 10 },
      columns: [
        { id: 'todo', name: 'To Do', wip_limit: null, issue_ids: ['i1'] },
        { id: 'done', name: 'Done', wip_limit: null, issue_ids: [] },
      ],
      issues: [
        {
          id: 'i1',
          key: 'TT-1',
          summary: 'Do work',
          priority: 'High',
          issue_type: 'Task',
          status_id: 'todo',
          assignee_name: 'me',
        },
      ],
    }),
  ),
)
const moveIssue = vi.hoisted(() => vi.fn(() => Promise.resolve()))

vi.mock('@/api/board', () => ({
  getBoard,
  moveIssue,
}))

function wrapper(children: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return (
    <ThemeProvider>
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={['/projects/TT/board']}>
          <Routes>
            <Route path="/projects/:projectKey/board" element={children} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>
    </ThemeProvider>
  )
}

describe('ProjectBoardPage', () => {
  it('renders board columns and issue card', async () => {
    render(wrapper(<ProjectBoardPage />))
    await waitFor(() => expect(screen.getByText('TT Kanban · Sprint 1')).toBeInTheDocument())
    const columns = screen.getAllByText('To Do')
    expect(columns.length).toBeGreaterThanOrEqual(1)
    const keys = screen.getAllByText('TT-1')
    expect(keys.length).toBeGreaterThanOrEqual(1)
  })
})
