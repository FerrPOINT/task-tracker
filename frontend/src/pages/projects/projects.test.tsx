import { describe, it, expect, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import { ProjectsPage } from './'
import { ThemeProvider } from '@/shared/lib/theme'

vi.mock('@/shared/api/hooks', () => ({
  useProjects: () => ({
    data: [
      {
        id: 'p1',
        key: 'TT',
        name: 'Task Tracker',
        owner_id: 'u1',
        todo_count: 2,
        in_progress_count: 3,
        done_count: 1,
      },
    ],
    isLoading: false,
    error: null,
  }),
  useCreateProject: () => ({ mutate: vi.fn(), isPending: false, error: null }),
  useUpdateProject: () => ({ mutate: vi.fn(), isPending: false, error: null }),
  useDeleteProject: () => ({ mutate: vi.fn(), isPending: false, error: null }),
}))

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

describe('ProjectsPage', () => {
  it('renders project list', async () => {
    render(wrapper(<ProjectsPage />))
    await waitFor(() => expect(screen.getByText('Task Tracker')).toBeInTheDocument())
    expect(screen.getByText(/к выполнению|todo/i)).toBeInTheDocument()
  })
})
