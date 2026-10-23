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
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Routes, Route, useLocation } from 'react-router'

import { ProjectBoardPage } from './'
import { ThemeProvider } from '@pdlc/ui/lib'

const navigateToFleet = vi.hoisted(() => vi.fn())
const boardHooks = vi.hoisted(() => ({
  move: vi.fn(),
  transitions: [] as Array<{ from_status_id: string; to_status_id: string }>,
}))
beforeEach(() => {
  boardHooks.move.mockReset()
  boardHooks.transitions = []
})
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
      sprint: { id: 's1', name: 'Sprint 1', remaining_days: 10 },
      columns: [
        { id: 'todo', name: 'To Do', wip_limit: null, issue_ids: ['i1'] },
        { id: 'done', name: 'Done', wip_limit: null, issue_ids: [] },
      ],
      issues: [
        {
          id: 'i1',
          key: 'TT-1',
          summary: 'Do work',
          priority: 'High',
          issue_type: 'Task',
          status_id: 'todo',
          assignee_name: 'me',
        },
      ],
    },
    isLoading: false,
    error: null,
  }),
  useMoveIssue: () => ({ mutate: boardHooks.move, isPending: false }),
  useTransitions: () => ({ data: boardHooks.transitions, isLoading: false }),
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
  it.each([false, true])(
    'keeps one native drop and owner transition checks (allowed=%s)',
    (allowed) => {
      boardHooks.transitions = allowed ? [{ from_status_id: 'todo', to_status_id: 'done' }] : []
      render(wrapper(<ProjectBoardPage />))
      const article = screen.getByRole('article')
      const board = screen.getByRole('region', { name: 'Колонки доски' })
      const target = within(board)
        .getByText(/Готово|Done/)
        .closest('.base-card')!
      const transfer = { effectAllowed: '', setData: vi.fn(), getData: vi.fn(() => 'i1') }
      expect(article).toHaveAttribute('draggable', 'true')
      expect(article).not.toHaveAttribute('data-interactive')
      expect(boardHooks.move).not.toHaveBeenCalled()
      fireEvent.dragStart(article, { dataTransfer: transfer })
      expect(transfer.setData).toHaveBeenCalledExactlyOnceWith('text/plain', 'i1')
      fireEvent.dragOver(target, { dataTransfer: transfer })
      expect(target).toHaveAttribute('data-highlighted', 'true')
      expect(boardHooks.move).not.toHaveBeenCalled()
      fireEvent.drop(target, { dataTransfer: transfer })
      expect(target).not.toHaveAttribute('data-highlighted')
      if (allowed)
        expect(boardHooks.move).toHaveBeenCalledExactlyOnceWith(
          { issue_id: 'i1', status_id: 'done' },
          expect.any(Object),
        )
      else expect(boardHooks.move).not.toHaveBeenCalled()
    },
  )
  it('changes the visible status through the shared group without losing URL context or starting work', async () => {
    function Address() {
      const location = useLocation()
      return <output aria-label="Current URL">{location.pathname + location.search}</output>
    }
    navigateToFleet.mockClear()
    const user = userEvent.setup()
    render(
      wrapper(
        <>
          <ProjectBoardPage />
          <Address />
        </>,
        '/projects/TT/board?source=qa&status=todo&registry_instance_id=registry-qa&namespace_id=ns-qa',
      ),
    )
    const group = screen.getByRole('radiogroup', { name: 'Статус задач' })
    expect(within(group).getByRole('radio', { name: /К выполнению|To Do/i })).toBeChecked()
    await user.click(within(group).getByRole('radio', { name: /Готово|Done/i }))
    await waitFor(() =>
      expect(screen.getByLabelText('Current URL')).toHaveTextContent('status=done'),
    )
    expect(screen.getByLabelText('Current URL')).toHaveTextContent('source=qa')
    expect(screen.getByLabelText('Current URL')).toHaveTextContent(
      'registry_instance_id=registry-qa',
    )
    expect(screen.getByLabelText('Current URL')).toHaveTextContent('namespace_id=ns-qa')
    expect(within(group).getByRole('radio', { name: /Готово|Done/i })).toBeChecked()
    expect(navigateToFleet).not.toHaveBeenCalled()
  })
  it('opens empty PM chat with the same immutable project from every board column', async () => {
    navigateToFleet.mockClear()
    const user = userEvent.setup()
    render(wrapper(<ProjectBoardPage />))
    const actions = screen.getAllByRole('button', { name: 'Создать' })
    expect(actions).toHaveLength(2)
    for (const action of actions) await user.click(action)
    await waitFor(() => expect(navigateToFleet).toHaveBeenCalledTimes(2))
    expect(navigateToFleet).toHaveBeenCalledWith(
      'https://fleet.test/chats/new?project=68be198d-aabb-4d81-91bc-ac51ff621643',
    )
  })
  it('renders board columns and issue card', async () => {
    render(wrapper(<ProjectBoardPage />))
    const board = screen.getByRole('region', { name: 'Колонки доски' })
    expect(board).toHaveAttribute('tabindex', '0')
    expect(board).toHaveClass('md:grid-cols-2', 'xl:grid-cols-4')
    expect(board).not.toHaveClass('overflow-x-auto')
    const columns = await screen.findAllByText(/К выполнению/i)
    expect(columns.length).toBeGreaterThanOrEqual(1) // single responsive tree
    expect(screen.getAllByText('Do work').length).toBeGreaterThanOrEqual(1)
    expect(screen.queryByRole('button', { name: /участники|members/i })).not.toBeInTheDocument()
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
    expect(screen.getByRole('button', { name: 'Изменить статус TT-1' })).toHaveClass('h-10', 'w-10')
  })

  it('restores compact density from the board URL', () => {
    render(wrapper(<ProjectBoardPage />, '/projects/TT/board?density=compact'))

    expect(screen.getByRole('radio', { name: 'Компактная' })).toBeChecked()
    expect(
      screen.getByText('Do work').closest('article')?.querySelector('.base-card-content'),
    ).toHaveAttribute('data-density', 'compact')
  })
})
