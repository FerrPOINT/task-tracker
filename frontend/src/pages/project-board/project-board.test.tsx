import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter, Routes, Route } from 'react-router'

import { ProjectBoardPage } from './'
import { ThemeProvider } from '@sdlc/ui/lib'

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
  useTransitions: () => ({ data: [], isLoading: false }),
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
    const board = screen.getByRole('region', { name: 'Колонки доски' })
    expect(board).toHaveAttribute('tabindex', '0')
    expect(board).toHaveClass('md:grid-cols-2', 'xl:grid-cols-4')
    expect(board).not.toHaveClass('overflow-x-auto')
    const columns = await screen.findAllByText(/К выполнению/i)
    expect(columns.length).toBeGreaterThanOrEqual(1) // single responsive tree
    expect(screen.getAllByText('Do work').length).toBeGreaterThanOrEqual(1)
    expect(screen.queryByRole('button', { name: /участники|members/i })).not.toBeInTheDocument()
  })
})
