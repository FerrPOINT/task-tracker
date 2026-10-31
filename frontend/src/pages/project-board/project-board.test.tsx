import { beforeEach, describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Routes, Route, useLocation } from 'react-router'
import { ThemeProvider } from '@pdlc/ui/lib'
import { choiceOptions, chooseValue } from '@/test/choice'
import { ProjectBoardPage } from './'
import type { StageBoard } from '@/api/task-lifecycle'

const state = vi.hoisted(() => ({ change: vi.fn(), allowed: true, active: true }))
const stages = [
  'Draft',
  'Backlog',
  'Analysis',
  'Architecture',
  'Development',
  'Review',
  'Testing',
  'Rework',
  'Deployment',
] as const
function fixture(): StageBoard {
  return {
    project_key: 'TT',
    project_id: 'project',
    backlog_total: 0,
    columns: stages.map((stage) => ({
      stage,
      issue_ids: stage === 'Analysis' ? ['i1'] : [],
      wip_limit: null,
    })),
    issues: [
      {
        issue: {
          id: 'i1',
          key: 'TT-1',
          summary: 'Do work',
          description: 'Materials',
          status: 'InProgress',
          status_id: 'internal',
          priority: 'High',
          issue_type: 'Task',
          project_key: 'TT',
          project_name: 'Tracker',
          reporter_id: 'author',
          created_at: '2026-10-30T00:00:00Z',
          updated_at: '2026-10-30T00:00:00Z',
          labels: [],
          time_spent_seconds: 0,
        },
        lifecycle: {
          task_id: 'i1',
          state: { stage: 'Analysis', status: 'InProgress', revision: 7 },
          execution_busy: false,
          allowed_actions: state.allowed
            ? [
                {
                  action: 'complete',
                  target: { stage: 'Architecture', status: 'Ready', revision: 8 },
                },
              ]
            : [],
        },
      },
    ],
    sprint: { id: 'none', name: 'Backlog', goal: '', state: 'future', velocity: 0, issue_ids: [] },
  }
}
vi.mock('@/features/issues/model/task-lifecycle', () => ({
  useTaskLifecycleCatalog: () => ({ data: { enabled: state.active } }),
  useStageBoard: () => ({ data: fixture() }),
  useLifecycleTransition: () => ({ mutate: state.change, isPending: false }),
}))
beforeEach(() => {
  state.change.mockReset()
  state.allowed = true
  state.active = true
})
function Address() {
  const value = useLocation()
  return <output aria-label="Current URL">{value.pathname + value.search}</output>
}
function mount(entry = '/projects/TT/board?stage=Analysis') {
  return render(
    <ThemeProvider>
      <MemoryRouter initialEntries={[entry]}>
        <Routes>
          <Route path="/projects/:projectKey/board" element={<ProjectBoardPage />} />
          <Route path="/issues/create" element={<h1>Ordinary creation</h1>} />
        </Routes>
        <Address />
      </MemoryRouter>
    </ThemeProvider>,
  )
}

describe('mandatory project stage board', () => {
  it.each([false, true])(
    'performs exactly one native drop only when the owner allows it (%s)',
    (allowed) => {
      state.allowed = allowed
      mount()
      const article = screen.getByRole('article')
      const board = screen.getByRole('region', { name: 'Стадии задач' })
      const target = within(board)
        .getByRole('heading', { name: 'Архитектура' })
        .closest('.base-card')!
      const transfer = { effectAllowed: '', setData: vi.fn(), getData: vi.fn(() => 'i1') }
      expect(article).toHaveAttribute('draggable', String(allowed))
      expect(article).not.toHaveAttribute('data-interactive')
      fireEvent.dragStart(article, { dataTransfer: transfer })
      fireEvent.dragOver(target, { dataTransfer: transfer })
      expect(state.change).not.toHaveBeenCalled()
      if (allowed) expect(target).toHaveAttribute('data-highlighted', 'true')
      else expect(target).not.toHaveAttribute('data-highlighted')
      fireEvent.drop(target, { dataTransfer: transfer })
      expect(target).not.toHaveAttribute('data-highlighted')
      if (allowed)
        expect(state.change).toHaveBeenCalledExactlyOnceWith(
          {
            id: 'i1',
            command: {
              action: 'complete',
              expected_revision: 7,
              idempotency_key: expect.any(String),
            },
          },
          expect.any(Object),
        )
      else expect(state.change).not.toHaveBeenCalled()
      expect(screen.getByText('Do work')).toBeInTheDocument()
    },
  )
  it('provides all nine stages and changes the mobile view without losing scope or starting work', async () => {
    mount(
      '/projects/TT/board?stage=Analysis&source=qa&registry_instance_id=registry-qa&namespace_id=ns-qa',
    )
    const select = screen.getByRole('combobox', { name: 'Стадия задач' })
    expect((await choiceOptions(select)).map((item) => item.value)).toEqual([...stages])
    await chooseValue(select, 'Deployment')
    const address = screen.getByLabelText('Current URL')
    for (const value of [
      'stage=Deployment',
      'source=qa',
      'registry_instance_id=registry-qa',
      'namespace_id=ns-qa',
    ])
      expect(address).toHaveTextContent(value)
    expect(state.change).not.toHaveBeenCalled()
  })
  it('opens ordinary creation for this project without requiring Fleet', async () => {
    mount()
    await userEvent.click(screen.getByRole('button', { name: /Создать|Create/i }))
    expect(screen.getByRole('heading', { name: 'Ordinary creation' })).toBeVisible()
    expect(screen.getByLabelText('Current URL')).toHaveTextContent(
      '/issues/create?source=navigation&project_key=TT',
    )
    expect(state.change).not.toHaveBeenCalled()
  })
  it('renders all stage columns and keeps the board keyboard focusable', () => {
    mount()
    const board = screen.getByRole('region', { name: 'Стадии задач' })
    expect(board).toHaveAttribute('tabindex', '0')
    expect(within(board).getAllByRole('heading', { level: 2 })).toHaveLength(9)
    expect(screen.getByText('Do work')).toBeVisible()
    expect(screen.queryByRole('button', { name: /участники|members/i })).not.toBeInTheDocument()
  })
  it('changes density without reducing the operation buttons or losing URL context', async () => {
    mount('/projects/TT/board?stage=Analysis&source=qa')
    const action = screen.getByRole('button', { name: 'Завершить этап' })
    const classes = action.className
    const article = screen.getByText('Do work').closest('article')!
    expect(article.querySelector('.base-card-content')).toHaveAttribute('data-density', 'default')
    await userEvent.click(screen.getByRole('radio', { name: 'Компактная' }))
    expect(article.querySelector('.base-card-content')).toHaveAttribute('data-density', 'compact')
    expect(action.className).toBe(classes)
    expect(screen.getByLabelText('Current URL')).toHaveTextContent('source=qa')
  })
  it('restores compact density from the URL', () => {
    mount('/projects/TT/board?density=compact&stage=Analysis')
    expect(screen.getByRole('radio', { name: 'Компактная' })).toBeChecked()
    expect(screen.getByRole('article').querySelector('.base-card-content')).toHaveAttribute(
      'data-density',
      'compact',
    )
  })
  it('never offers a legacy board writer while the coordinated migration is pending', () => {
    state.active = false
    mount()
    expect(
      screen.getByText(
        'Модель задач обновляется. Доска станет доступна после завершения переноса.',
      ),
    ).toBeVisible()
    expect(screen.queryByRole('article')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /Создать|Create/i })).not.toBeInTheDocument()
  })
})
