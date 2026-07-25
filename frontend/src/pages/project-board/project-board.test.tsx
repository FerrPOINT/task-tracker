import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter, Routes, Route } from 'react-router'

import { ProjectBoardPage } from './'
import { ThemeProvider } from '@/shared/lib/theme'

vi.mock('@/shared/api/hooks', () => ({
  useBoard: () => ({
    data: {
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
    },
    isLoading: false,
    error: null,
  }),
  useMoveIssue: () => ({ mutate: vi.fn(), isPending: false }),
}))

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter initialEntries={['/projects/TT/board']}>
        <Routes>
          <Route path="/projects/:projectKey/board" element={children} />
        </Routes>
      </MemoryRouter>
    </ThemeProvider>
  )
}

describe('ProjectBoardPage', () => {
  it('renders board columns and issue card', async () => {
    render(wrapper(<ProjectBoardPage />))
    const columns = await screen.findAllByText(/to do/i)
    expect(columns.length).toBeGreaterThanOrEqual(2)
    expect(screen.getAllByText('Do work').length).toBeGreaterThanOrEqual(2)
  })
})
