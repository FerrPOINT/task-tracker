import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'

import { DashboardPage } from './'
import { ThemeProvider } from '@sdlc/ui/lib'
import { useAuthStore } from '@/shared/auth/store'

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

const listProjects = vi.hoisted(() =>
  vi.fn(() =>
    Promise.resolve([
      {
        id: 'p1',
        key: 'DEMO',
        name: 'Demo Project',
        todo_count: 1,
        in_progress_count: 2,
        done_count: 3,
      },
    ]),
  ),
)

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
  beforeEach(() => {
    vi.clearAllMocks()
    useAuthStore.setState({ token: 'tok', userId: 'u1', email: 'a@b' })
  })

  it('renders dashboard widgets and assigned issues', async () => {
    render(wrapper(<DashboardPage />))
    expect(screen.getByText(/командный дашборд|team dashboard/i)).toBeInTheDocument()
    await screen.findByText('Fix tests')
    expect(screen.getByText('В работе')).toBeInTheDocument()
    expect(screen.getByText('К выполнению: 1')).toBeInTheDocument()
    expect(screen.getByText('В работе: 2')).toBeInTheDocument()
    expect(screen.getByText('Готово: 3')).toBeInTheDocument()
  })

  it('keeps the project list available when assigned issues fail to load', async () => {
    getDashboard.mockRejectedValueOnce(new Error('Failed to load dashboard'))
    const user = userEvent.setup()
    render(wrapper(<DashboardPage />))

    await screen.findByText('Не удалось загрузить назначенные задачи. Повторите попытку.')
    expect(screen.getByText('DEMO · Demo Project')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /повторить/i }))
    await screen.findByText('Fix tests')
  })

  it('keeps assigned issues available when projects fail to load', async () => {
    listProjects.mockRejectedValueOnce(new Error('Failed to load projects'))
    render(wrapper(<DashboardPage />))

    await screen.findByText('Не удалось загрузить проекты. Повторите попытку.')
    expect(screen.getByText('Fix tests')).toBeInTheDocument()
    expect(screen.queryByText('Проекты · 0')).not.toBeInTheDocument()
  })

  it('shows a bounded overview with links to complete lists', async () => {
    const longSummary =
      'Длинное название задачи для проверки переноса строки на узком экране и удобного сканирования'
    getDashboard.mockResolvedValueOnce({
      assigned_issues: Array.from({ length: 12 }, (_, index) => ({
        id: `i${index + 1}`,
        key: `TT-${index + 1}`,
        summary: index === 0 ? longSummary : `Issue ${index + 1}`,
        status: 'To Do',
        priority: 'High',
        assignee_name: 'me',
      })),
      recent_worklogs: [],
    })
    listProjects.mockResolvedValueOnce(
      Array.from({ length: 12 }, (_, index) => ({
        id: `p${index + 1}`,
        key: `P${index + 1}`,
        name: `Project ${index + 1}`,
        todo_count: 1,
        in_progress_count: 2,
        done_count: 3,
      })),
    )
    render(wrapper(<DashboardPage />))

    await screen.findByText(longSummary)
    expect(screen.getByText(longSummary)).toHaveClass('line-clamp-2')
    expect(screen.getByRole('link', { name: /Все мои задачи/i })).toHaveAttribute(
      'href',
      '/search?assignee_id=u1',
    )
    expect(screen.getByRole('link', { name: /Все проекты/i })).toHaveAttribute('href', '/projects')
    expect(screen.getByRole('link', { name: /TT-5/ })).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: /TT-6/ })).not.toBeInTheDocument()
    expect(screen.getByRole('link', { name: /P5 · Project 5/ })).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: /P6 · Project 6/ })).not.toBeInTheDocument()
  })
})
