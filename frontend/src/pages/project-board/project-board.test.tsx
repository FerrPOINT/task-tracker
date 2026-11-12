import { applicationAccessFixture } from '@/test/application-access'
import { beforeEach, describe, it, expect, vi } from 'vitest'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Routes, Route } from 'react-router'
import { ThemeProvider } from '@pdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { ProjectBoardPage } from './'

const accessActor = vi.hoisted(() => ({
  role: 'user' as 'user' | 'admin',
  applications: ['task-tracker', 'fleet-control'],
}))
vi.mock('@pdlc/ui/auth', async (original) => ({
  ...(await original<object>()),
  useApplicationAccess: () => applicationAccessFixture(accessActor),
}))
const fixture = vi.hoisted(() => ({
  empty: false,
  failed: false,
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
  ],
}))
const navigateToFleet = vi.hoisted(() => vi.fn())
const move = vi.hoisted(() => vi.fn())
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
  useBoard: (key: string) => ({
    data: {
      project_key: key,
      sprint: { id: 's1', name: 'Sprint 1', remaining_days: 10 },
      // These are historical statuses, deliberately unrelated to SDLC progress.
      columns: [
        { id: 'todo', name: 'To Do', wip_limit: null, issue_ids: ['i1', 'i2'] },
        { id: 'done', name: 'Done', wip_limit: null, issue_ids: ['i3'] },
      ],
      sdlc: {
        stages: fixture.stages,
        tasks: fixture.empty
          ? []
          : [
              { issue_id: 'i1', stage: 'Development', status: fixture.status },
              { issue_id: 'i2', stage: 'Review', status: 'Ready' },
              { issue_id: 'i3', stage: 'Draft' },
            ],
      },
      issues: fixture.empty
        ? []
        : [
            {
              id: 'i1',
              key: key + '-1',
              summary: 'Do work',
              priority: 'High',
              issue_type: 'Task',
              status_id: 'todo',
            },
            {
              id: 'i2',
              key: key + '-2',
              summary: 'Review work',
              priority: 'High',
              issue_type: 'Task',
              status_id: 'todo',
            },
            {
              id: 'i3',
              key: key + '-3',
              summary: 'Historical work',
              priority: 'High',
              issue_type: 'Task',
              status_id: 'done',
              status: 'Done',
            },
          ],
    },
    isLoading: false,
    error: fixture.failed ? new Error('Failed to load board') : null,
  }),
  useMoveIssue: () => ({ mutate: move, isPending: false }),
  useTransitions: () => ({ data: [], isLoading: false }),
}))
function wrapper(key = 'TT', search = '') {
  return (
    <ThemeProvider>
      <MemoryRouter initialEntries={['/projects/' + key + '/board' + search]}>
        <Routes>
          <Route path="/projects/:projectKey/board" element={<ProjectBoardPage />} />
        </Routes>
      </MemoryRouter>
    </ThemeProvider>
  )
}
const expected = [
  'Черновик',
  'Бэклог',
  'Анализ',
  'Архитектура',
  'Разработка',
  'Ревью',
  'Тестирование',
  'Доработка',
  'Поставка',
]
describe('Canonical project board', () => {
  beforeEach(async () => {
    fixture.empty = false
    fixture.failed = false
    fixture.status = 'Ready'
    move.mockReset()
    navigateToFleet.mockReset()
    await i18n.changeLanguage('ru')
  })
  it.each(['TT', 'NATIVE', 'EMPTY'])('shows canonical stages for every project (%s)', (key) => {
    fixture.empty = key === 'EMPTY'
    render(wrapper(key))
    const board = screen.getByRole('region', { name: 'Колонки доски' })
    expect(
      within(board)
        .getAllByRole('heading', { level: 2 })
        .map((x) => x.textContent),
    ).toEqual(expected)
    expect(within(board).queryByText('К выполнению', { exact: true })).not.toBeInTheDocument()
    expect(within(board).queryByText('Готово', { exact: true })).not.toBeInTheDocument()
  })
  it('keeps every authorized card and separates stage from legacy status', () => {
    render(wrapper())
    for (const [stage, summary] of [
      ['Разработка', 'Do work'],
      ['Ревью', 'Review work'],
      ['Черновик', 'Historical work'],
    ] as const) {
      expect(screen.getByRole('heading', { name: stage }).closest('section')).toHaveTextContent(
        summary,
      )
    }
    expect(screen.getAllByRole('article')).toHaveLength(3)
    for (const card of screen.getAllByRole('article')) expect(card).not.toHaveAttribute('draggable')
    expect(screen.queryByRole('button', { name: /Изменить статус/ })).not.toBeInTheDocument()
    expect(move).not.toHaveBeenCalled()
  })
  it.each([
    ['Ready', 'Ожидает выполнения'],
    ['InProgress', 'В работе'],
    ['Completed', 'Этап завершён'],
  ] as const)('shows the independent %s state on its card', (status, label) => {
    fixture.status = status
    render(wrapper())
    const card = screen.getByRole('link', { name: 'TT-1 Do work' })
    expect(card).toHaveAttribute('href', '/issues/i1')
    expect(within(card).getByText(label)).toBeInTheDocument()
  })
  it('does not invent execution for an unstarted task', () => {
    fixture.status = undefined
    render(wrapper())
    const card = screen.getByRole('link', { name: 'TT-1 Do work' })
    expect(within(card).queryByText('Ожидает выполнения')).not.toBeInTheDocument()
  })
  it('does not hide columns or lose scoped card links for an old status URL', () => {
    render(wrapper('TT', '?status=done&registry_instance_id=registry-qa&namespace_id=ns-qa'))
    expect(screen.getAllByRole('heading', { level: 2 }).map((x) => x.textContent)).toEqual(expected)
    expect(screen.getByRole('link', { name: 'TT-1 Do work' })).toHaveAttribute(
      'href',
      '/issues/i1?registry_instance_id=registry-qa&namespace_id=ns-qa',
    )
    expect(navigateToFleet).not.toHaveBeenCalled()
  })
  it('reports a failed board instead of showing an empty successful board', () => {
    fixture.failed = true
    render(wrapper())
    expect(screen.getByRole('alert')).toHaveTextContent('Failed to load board')
    expect(screen.queryByRole('region', { name: 'Колонки доски' })).not.toBeInTheDocument()
  })
  it('opens the existing PM entry point with the immutable project ID', async () => {
    const user = userEvent.setup()
    render(wrapper())
    await user.click(screen.getByRole('button', { name: 'Создать' }))
    await waitFor(() =>
      expect(navigateToFleet).toHaveBeenCalledExactlyOnceWith(
        'https://fleet.test/chats/new?project=68be198d-aabb-4d81-91bc-ac51ff621643',
      ),
    )
  })
})
