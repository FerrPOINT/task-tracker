import { applicationAccessFixture } from '@/test/application-access'

const accessActor = vi.hoisted(() => ({
  role: 'user' as 'user' | 'admin',
  applications: ['task-tracker', 'fleet-control'],
}))
vi.mock('@pdlc/ui/auth', async (original) => ({
  ...(await original<object>()),
  useApplicationAccess: () => applicationAccessFixture(accessActor),
}))
import { describe, it, expect, vi } from 'vitest'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Routes, Route } from 'react-router'

import { ProjectBoardPage } from './'
import { ThemeProvider } from '@pdlc/ui/lib'
import type { components } from '@/api/generated'

const fixture = vi.hoisted(() => ({
  empty: false,
  stages: [
    'Draft', 'Backlog', 'Analysis', 'Architecture', 'Development', 'Review',
    'Testing', 'Rework', 'Deployment',
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
      sdlc: {
        stages: fixture.stages,
        tasks: fixture.empty ? [] : [
          { issue_id: 'i1', stage: 'Development', status: 'Ready' },
          { issue_id: 'i2', stage: 'Review', status: 'Ready' },
        ],
      },
      sprint: { id: 's1', name: 'Sprint 1', remaining_days: 10 },
      columns: [
        { id: 'todo', name: 'To Do', wip_limit: null, issue_ids: ['i1'] },
        { id: 'done', name: 'Done', wip_limit: null, issue_ids: [] },
      ],
      issues: fixture.empty ? [] : [
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

  it('opens empty PM chat with the same immutable project only from Draft', async () => {
    navigateToFleet.mockClear()
    const user = userEvent.setup()
    render(wrapper(<ProjectBoardPage />))
    const actions = screen.getAllByRole('button', { name: 'Создать' })
    expect(actions).toHaveLength(1)
    for (const action of actions) await user.click(action)
    await waitFor(() => expect(navigateToFleet).toHaveBeenCalledTimes(1))
    expect(navigateToFleet).toHaveBeenCalledWith(
      'https://fleet.test/chats/new?project=68be198d-aabb-4d81-91bc-ac51ff621643',
    )
  })
  it('renders all nine stages on an empty project', () => {
    fixture.empty = true
    try {
      render(wrapper(<ProjectBoardPage />))
      const board = screen.getByRole('region', { name: 'Колонки доски' })
      expect(within(board).getAllByRole('heading', { level: 2 }).map((heading) => heading.textContent))
        .toEqual(['Черновик', 'Бэклог', 'Анализ', 'Архитектура', 'Разработка', 'Ревью',
          'Тестирование', 'Доработка', 'Развёртывание'])
      expect(within(board).queryByRole('article')).not.toBeInTheDocument()
    } finally {
      fixture.empty = false
    }
  })

  it('switches to compact density without shrinking issue actions', async () => {
    const user = userEvent.setup()
    render(wrapper(<ProjectBoardPage />))

    const compact = screen.getByRole('radio', { name: 'Компактная' })
    const comfortable = screen.getByRole('radio', { name: 'Обычная' })
    const issueCard = screen.getByText('Do work').closest('article')

    expect(comfortable).toBeChecked()
    expect(issueCard?.querySelector('.base-card-content')).toHaveAttribute(
      'data-density',
      'default',
    )

    await user.click(compact)

    expect(compact).toBeChecked()
    expect(issueCard?.querySelector('.base-card-content')).toHaveAttribute(
      'data-density',
      'compact',
    )
    expect(screen.queryByRole('button', { name: 'Изменить статус TT-1' })).not.toBeInTheDocument()
  })

  it('restores compact density from the board URL', () => {
    render(wrapper(<ProjectBoardPage />, '/projects/TT/board?density=compact'))

    expect(screen.getByRole('radio', { name: 'Компактная' })).toBeChecked()
    expect(
      screen.getByText('Do work').closest('article')?.querySelector('.base-card-content'),
    ).toHaveAttribute('data-density', 'compact')
  })
})
