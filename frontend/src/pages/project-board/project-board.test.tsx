import { applicationAccessFixture } from '@/test/application-access'

const accessActor = vi.hoisted(() => ({
  role: 'user' as 'user' | 'admin',
  applications: ['task-tracker', 'fleet-control'],
}))
vi.mock('@pdlc/ui/auth', async (original) => ({
  ...(await original<object>()),
  useApplicationAccess: () => applicationAccessFixture(accessActor),
}))
import { beforeEach, describe, it, expect, vi } from 'vitest'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Routes, Route } from 'react-router'

import { ProjectBoardPage } from './'
import { ThemeProvider } from '@pdlc/ui/lib'
import type { components } from '@/api/generated'

const fixture = vi.hoisted(() => ({
  empty: false,
  configured: true,
  status: 'Ready' as 'Ready' | 'InProgress' | 'Completed' | undefined,
  stages: [
    'Draft',
    'Backlog',
    'Analysis',
    'Architecture',
    'Development',
    'Review',
    'Testing',
    'Rework',
    'Deployment',
  ] as components['schemas']['SdlcBoardResponse']['stages'],
}))
const navigateToFleet = vi.hoisted(() => vi.fn())
vi.mock('@pdlc/ui/platform', async (original) => ({
  ...(await original<object>()),
  platformNavigate: navigateToFleet,
}))
vi.mock('@pdlc/ui/ui', async (original) => ({
  ...(await original<object>()),
  usePlatformServices: () => ({
    source: 'runtime',
    services: [{ key: 'fleet-control', ui_url: 'https://fleet.test' }],
  }),
}))
vi.mock('@/api/project', () => ({
  getProject: async (key: string) => ({ key, id: '68be198d-aabb-4d81-91bc-ac51ff621643' }),
}))

vi.mock('@/shared/api/hooks', () => ({
  useBoard: () => ({
    data: {
      project_key: 'TT',
      sdlc: fixture.configured
        ? {
            stages: fixture.stages,
            tasks: fixture.empty
              ? []
              : [
                  { issue_id: 'i1', stage: 'Development', status: fixture.status },
                  { issue_id: 'i2', stage: 'Review', status: 'Ready' },
                ],
          }
        : null,
      sprint: { id: 's1', name: 'Sprint 1', remaining_days: 10 },
      columns: [
        { id: 'todo', name: 'To Do', wip_limit: null, issue_ids: ['i1'] },
        { id: 'done', name: 'Done', wip_limit: null, issue_ids: [] },
      ],
      issues: fixture.empty
        ? []
        : [
            {
              id: 'i1',
              key: 'TT-1',
              summary: 'Do work',
              priority: 'High',
              issue_type: 'Task',
              status_id: 'todo',
              assignee_name: 'me',
            },
            ...(!fixture.empty
              ? [
                  {
                    id: 'i2',
                    key: 'TT-2',
                    summary: 'Review work',
                    priority: 'High',
                    issue_type: 'Task',
                    status_id: 'todo',
                  },
                  {
                    id: 'i3',
                    key: 'TT-3',
                    summary: 'Ordinary work',
                    priority: 'High',
                    issue_type: 'Task',
                    status_id: 'todo',
                    status: 'To Do',
                  },
                ]
              : []),
          ],
    },
    isLoading: false,
    error: null,
  }),
}))

function wrapper(children: React.ReactNode, initialEntry = '/projects/TT/board') {
  return (
    <ThemeProvider>
      <MemoryRouter initialEntries={[initialEntry]}>
        <Routes>
          <Route path="/projects/:projectKey/board" element={children} />
        </Routes>
      </MemoryRouter>
    </ThemeProvider>
  )
}

describe('ProjectBoardPage', () => {
  beforeEach(() => {
    fixture.status = 'Ready'
    fixture.configured = true
  })

  it('groups the same issue status into separate SDLC stages without displaying ordinary cards', () => {
    render(wrapper(<ProjectBoardPage />))
    for (const [stage, card] of [
      ['Разработка', 'Do work'],
      ['Ревью', 'Review work'],
    ] as const) {
      const section = screen.getByRole('heading', { name: stage }).closest('section')
      expect(section).toHaveTextContent(card)
      expect(screen.getByText(card).closest('article')).not.toHaveAttribute('draggable')
    }
    expect(screen.queryByText('Ordinary work')).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Доработка' })).toBeInTheDocument()
  })

  it('opens empty PM chat with the same immutable project from the board header', async () => {
    navigateToFleet.mockClear()
    const user = userEvent.setup()
    render(wrapper(<ProjectBoardPage />))
    const actions = screen.getAllByRole('button', { name: 'Новая задача' })
    expect(actions).toHaveLength(1)
    expect(
      within(screen.getByRole('region', { name: 'Колонки доски' })).queryByRole('button', {
        name: 'Новая задача',
      }),
    ).not.toBeInTheDocument()
    for (const action of actions) await user.click(action)
    await waitFor(() => expect(navigateToFleet).toHaveBeenCalledTimes(1))
    expect(navigateToFleet).toHaveBeenCalledWith(
      'https://fleet.test/chats/new?project=68be198d-aabb-4d81-91bc-ac51ff621643',
    )
  })

  it.each([
    ['Ready', 'Ожидает выполнения'],
    ['InProgress', 'В работе'],
    ['Completed', 'Этап завершён'],
  ] as const)('shows %s as the current stage state on the linked card', (status, label) => {
    fixture.status = status
    render(wrapper(<ProjectBoardPage />))
    const card = screen.getByRole('link', { name: 'TT-1 Do work' })
    expect(card).toHaveAttribute('href', '/issues/i1')
    expect(within(card).getByRole('heading', { name: 'Do work' })).toBeInTheDocument()
    expect(within(card).getByText(label)).toBeInTheDocument()
    expect(within(card).queryByText('Готово', { exact: true })).not.toBeInTheDocument()
    expect(within(card).queryByText('Задача', { exact: true })).not.toBeInTheDocument()
    expect(within(card).queryByRole('img')).not.toBeInTheDocument()
  })
  it('does not invent an operational status when the lifecycle has none', () => {
    fixture.status = undefined
    render(wrapper(<ProjectBoardPage />))
    const card = screen.getByRole('link', { name: 'TT-1 Do work' })
    expect(card).toBeVisible()
    expect(within(card).queryByText('Ожидает выполнения')).not.toBeInTheDocument()
  })

  it('reports an unconfigured SDLC explicitly instead of rendering status columns', () => {
    fixture.configured = false
    render(wrapper(<ProjectBoardPage />))
    expect(screen.getByRole('alert')).toHaveTextContent('SDLC не настроен')
    expect(screen.queryByRole('region', { name: 'Колонки доски' })).not.toBeInTheDocument()
    expect(screen.queryByText('Ordinary work')).not.toBeInTheDocument()
  })

  it('renders all nine stages on an empty project', () => {
    fixture.empty = true
    try {
      render(wrapper(<ProjectBoardPage />))
      const board = screen.getByRole('region', { name: 'Колонки доски' })
      expect(
        within(board)
          .getAllByRole('heading', { level: 2 })
          .map((heading) => heading.textContent),
      ).toEqual([
        'Черновик',
        'Бэклог',
        'Анализ',
        'Архитектура',
        'Разработка',
        'Ревью',
        'Тестирование',
        'Доработка',
        'Развёртывание',
      ])
      expect(within(board).queryByRole('article')).not.toBeInTheDocument()
    } finally {
      fixture.empty = false
    }
  })

  it.each(['', '?density=compact', '?density=comfortable'])(
    'always renders compact cards without a density control (%s)',
    (search) => {
      render(wrapper(<ProjectBoardPage />, `/projects/TT/board${search}`))
      expect(
        screen.getByText('Do work').closest('article')?.querySelector('.base-card-content'),
      ).toHaveAttribute('data-density', 'compact')
      expect(screen.queryByRole('radiogroup', { name: 'Плотность доски' })).not.toBeInTheDocument()
    },
  )
})
