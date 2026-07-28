import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'

import { SearchPage } from './'

const search = vi.hoisted(() => vi.fn())
vi.mock('@/shared/api/hooks', () => ({ useSearch: search }))

function wrapper(children: React.ReactNode) {
  return <MemoryRouter>{children}</MemoryRouter>
}

describe('SearchPage', () => {
  it('renders search form and results', async () => {
    search.mockReturnValue({
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

    render(wrapper(<SearchPage />))
    expect(screen.getByText(/поиск задач|search issues/i)).toBeInTheDocument()
    expect(screen.getByText('TT-1')).toBeInTheDocument()

    const input = screen.getByDisplayValue('project = TT AND status != Done')
    await userEvent.clear(input)
    await userEvent.type(input, 'project = TT')

    await userEvent.click(screen.getByRole('button', { name: /найти|search/i }))
    expect(search).toHaveBeenCalled()
  })
})
