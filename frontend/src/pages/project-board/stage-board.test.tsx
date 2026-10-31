import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { MemoryRouter, Route, Routes } from 'react-router'
import { TaskStageBoard } from './stage-board'
import type { StageBoard } from '@/api/task-lifecycle'

const wire = vi.hoisted(() => {
  vi.stubEnv('VITE_API_BASE_URL', 'https://tracker.example.test/tracker')
  const fetch = vi.fn()
  vi.stubGlobal('fetch', fetch)
  return fetch
})
const taskId = '11111111-1111-4111-8111-111111111111'
let board: StageBoard
let requests: Request[]
let conflict = false
let lostResponse = false
function fixture(): StageBoard {
  return {
    project_id: '22222222-2222-4222-8222-222222222222',
    project_key: 'CORE',
    backlog_total: 0,
    columns: (
      [
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
    ).map((stage) => ({ stage, issue_ids: stage === 'Analysis' ? [taskId] : [] })),
    issues: [
      {
        issue: {
          id: taskId,
          key: 'CORE-1',
          summary: 'Сохранить данные',
          description: 'Материалы задачи',
          created_at: '2026-10-30T00:00:00Z',
          updated_at: '2026-10-30T00:00:00Z',
          status_id: 'projection',
          status: 'InProgress',
          priority: 'High',
          issue_type: 'Task',
          project_key: 'CORE',
          project_name: 'Core',
          reporter_id: 'author',
          labels: [],
          time_spent_seconds: 0,
        },
        lifecycle: {
          task_id: taskId,
          state: { stage: 'Analysis', status: 'InProgress', revision: 7 },
          execution_busy: false,
          allowed_actions: [
            { action: 'complete', target: { stage: 'Architecture', status: 'Ready', revision: 8 } },
          ],
        },
      },
    ],
    sprint: {
      id: '',
      name: '',
      goal: '',
      state: '',
      velocity: 0,
      remaining_days: 0,
      issue_ids: [],
    },
  }
}
function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={['/projects/CORE/board']}>
        <Routes>
          <Route path="/projects/:projectKey/board" element={<TaskStageBoard />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  )
}
beforeEach(() => {
  board = fixture()
  requests = []
  conflict = false
  lostResponse = false
  wire.mockImplementation(async (request: Request) => {
    requests.push(request)
    const path = new URL(request.url).pathname
    if (request.method === 'GET' && path === '/tracker/api/v2/projects/CORE/board')
      return Response.json(board)
    if (
      request.method === 'POST' &&
      path === `/tracker/api/v1/issues/${taskId}/lifecycle/transitions`
    ) {
      const card = board.issues[0]!
      card.lifecycle.state = { stage: 'Architecture', status: 'Ready', revision: conflict ? 9 : 8 }
      card.lifecycle.allowed_actions = []
      for (const column of board.columns)
        column.issue_ids = column.stage === 'Architecture' ? [taskId] : []
      if (lostResponse) throw new TypeError('Connection lost after server commit')
      return conflict
        ? Response.json({ error: { code: 'CONFLICT' } }, { status: 409 })
        : Response.json(card.lifecycle)
    }
    throw new Error(`Unexpected ${request.method} ${path}`)
  })
})
afterEach(() => {
  vi.clearAllMocks()
})

describe('stage board owner transport', () => {
  it('loads the actual project route and sends the server action and revision without legacy status writes', async () => {
    mount()
    await screen.findByText('Сохранить данные')
    expect(
      within(screen.getByRole('region', { name: 'Стадии задач' })).getAllByRole('heading'),
    ).toHaveLength(9)
    fireEvent.click(screen.getByRole('button', { name: 'Завершить этап' }))
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: 'Завершить этап' })).not.toBeInTheDocument(),
    )
    const posts = requests.filter((request) => request.method === 'POST')
    expect(posts).toHaveLength(1)
    expect(await posts[0]!.clone().json()).toMatchObject({
      action: 'complete',
      expected_revision: 7,
      idempotency_key: expect.any(String),
    })
    expect(requests.some((request) => request.url.includes('/board/move'))).toBe(false)
    const target = screen.getByRole('heading', { name: 'Архитектура' }).closest('.base-card')!
    expect(within(target as HTMLElement).getByText('Сохранить данные')).toBeInTheDocument()
  })
  it('keeps the task and refetches the actual column after a revision conflict', async () => {
    conflict = true
    mount()
    await screen.findByText('Сохранить данные')
    fireEvent.click(screen.getByRole('button', { name: 'Завершить этап' }))
    await waitFor(() =>
      expect(requests.filter((request) => request.method === 'GET').length).toBeGreaterThan(1),
    )
    expect(screen.getByText('Сохранить данные')).toBeInTheDocument()
    expect(requests.filter((request) => request.method === 'POST')).toHaveLength(1)
  })
  it('reads a committed result after transport loss without submitting another command', async () => {
    lostResponse = true
    mount()
    await screen.findByText('Сохранить данные')
    fireEvent.click(screen.getByRole('button', { name: 'Завершить этап' }))
    await waitFor(() => {
      const target = screen.getByRole('heading', { name: 'Архитектура' }).closest('.base-card')!
      expect(within(target as HTMLElement).getByText('Сохранить данные')).toBeInTheDocument()
    })
    expect(requests.filter((request) => request.method === 'POST')).toHaveLength(1)
    expect(requests.filter((request) => request.method === 'GET').length).toBeGreaterThan(1)
    expect(screen.getAllByText('Сохранить данные')).toHaveLength(1)
  })
  it('does not offer writes for unresolved agent custody even if its lease has expired', async () => {
    board.issues[0]!.lifecycle.execution_busy = true
    board.issues[0]!.lifecycle.allowed_actions = []
    mount()
    await screen.findByText('Сохранить данные')
    expect(screen.queryByRole('button', { name: 'Завершить этап' })).not.toBeInTheDocument()
    expect(screen.getByRole('article')).toHaveAttribute('draggable', 'false')
    expect(requests.every((request) => request.method === 'GET')).toBe(true)
  })
})
