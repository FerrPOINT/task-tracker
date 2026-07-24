import { describe, it, expect, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'

import { ProjectsPage } from './'
import { ThemeProvider } from '@/shared/lib/theme'

const listProjects = vi.hoisted(() =>
  vi.fn(() =>
    Promise.resolve([
      {
        id: 'p1',
        key: 'TT',
        name: 'Task Tracker',
        owner_id: 'u1',
        todo_count: 2,
        in_progress_count: 3,
        done_count: 1,
      },
    ]),
  ),
)

vi.mock('@/api/project', () => ({
  listProjects,
}))

function wrapper(children: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return (
    <ThemeProvider>
      <QueryClientProvider client={qc}>
        <MemoryRouter>{children}</MemoryRouter>
      </QueryClientProvider>
    </ThemeProvider>
  )
}

describe('ProjectsPage', () => {
  it('renders project list', async () => {
    render(wrapper(<ProjectsPage />))
    await waitFor(() => expect(screen.getByText('Task Tracker')).toBeInTheDocument())
    expect(screen.getByText('Todo')).toBeInTheDocument()
  })
})
