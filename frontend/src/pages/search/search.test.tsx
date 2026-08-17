import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import SearchPage from './'

const useIssues = vi.hoisted(() => vi.fn())
const useProjects = vi.hoisted(() => vi.fn())
const useUsers = vi.hoisted(() => vi.fn())
vi.mock('@/shared/api/hooks', () => ({
  useIssues,
  useProjects,
  useUsers,
}))

function wrapper(children: React.ReactNode) {
  return <MemoryRouter>{children}</MemoryRouter>
}

describe('SearchPage', () => {
  it('renders search form and results', async () => {
    useIssues.mockReturnValue({
      data: [
        {
          id: 'i1',
          key: 'TT-1',
          summary: 'Fix tests',
          status: 'In Progress',
          priority: 'High',
          assignee_name: 'Ivan',
        },
      ],
      isLoading: false,
      error: null,
    })
    useProjects.mockReturnValue({ data: [] })
    useUsers.mockReturnValue({ data: [] })

    render(wrapper(<SearchPage />))
    expect(screen.getByText(/поиск задач|search issues/i)).toBeInTheDocument()
    expect(screen.getByText('TT-1')).toBeInTheDocument()
  })
})
