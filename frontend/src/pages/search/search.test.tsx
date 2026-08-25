import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
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

function mockHooks() {
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
}

describe('SearchPage', () => {
  it('renders search form and results', () => {
    mockHooks()

    render(wrapper(<SearchPage />))

    expect(screen.getByText(/поиск задач|search issues/i)).toBeInTheDocument()
    expect(screen.getByText('TT-1')).toBeInTheDocument()
  })

  it('passes JQL input to the search hook', () => {
    mockHooks()

    render(wrapper(<SearchPage />))
    fireEvent.change(screen.getByPlaceholderText(/например: project|for example: project/i), {
      target: { value: 'project = TT' },
    })

    expect(useIssues).toHaveBeenLastCalledWith(expect.objectContaining({ jql: 'project = TT' }))
  })
})
