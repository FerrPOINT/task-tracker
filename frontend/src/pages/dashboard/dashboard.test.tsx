import { describe, it, expect, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'

import { DashboardPage } from './'
import { ThemeProvider } from '@/shared/lib/theme'

const getDashboard = vi.hoisted(() =>
  vi.fn(() =>
    Promise.resolve({
      assigned_issues: [
        {
          id: 'i1',
          key: 'TT-1',
          summary: 'Fix tests',
          status: 'In Progress',
          priority: 'High',
          assignee_name: 'me',
        },
      ],
      recent_worklogs: [],
    }),
  ),
)

const listProjects = vi.hoisted(() => vi.fn(() => Promise.resolve([])))

vi.mock('@/api/dashboard', () => ({
  getDashboard,
}))
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

describe('DashboardPage', () => {
  it('renders dashboard widgets and assigned issues', async () => {
    render(wrapper(<DashboardPage />))
    await waitFor(() => expect(screen.getByText('Team Dashboard')).toBeInTheDocument())
    expect(screen.getByText('TT-1 Fix tests')).toBeInTheDocument()
  })
})
