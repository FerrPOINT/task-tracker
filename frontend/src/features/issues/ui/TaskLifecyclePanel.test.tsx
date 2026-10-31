import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, fireEvent } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TaskLifecycleStatusGate, TaskLifecycleActions } from './TaskLifecyclePanel'

const wire = vi.hoisted(() => {
  vi.stubEnv('VITE_API_BASE_URL', 'https://tracker.example.test/tracker')
  const fetch = vi.fn()
  vi.stubGlobal('fetch', fetch)
  return fetch
})
let active = false
let unavailable = false
let requested: string[]
beforeEach(() => {
  active = false
  unavailable = false
  requested = []
  wire.mockImplementation(async (request: Request) => {
    const path = new URL(request.url).pathname
    requested.push(path)
    if (path === '/tracker/api/v1/task-lifecycle')
      return unavailable
        ? new Response(null, { status: 503 })
        : Response.json({ enabled: active, stages: [], statuses: [] })
    if (path === '/tracker/api/v1/issues/task/lifecycle')
      return Response.json({
        task_id: 'task',
        state: { stage: 'Development', status: 'Ready', revision: 9 },
        allowed_actions: [],
        execution_busy: false,
      })
    throw new Error(`Unexpected request ${path}`)
  })
})
afterEach(() => vi.clearAllMocks())
function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(
    <QueryClientProvider client={client}>
      <TaskLifecycleStatusGate issueId="task" />
    </QueryClientProvider>,
  )
}

describe('server activation boundary', () => {
  it('does not restore a legacy writer while the model is being migrated', async () => {
    mount()
    await screen.findByText('Модель задач обновляется. Изменение стадии временно недоступно.')
    expect(requested).toEqual(['/tracker/api/v1/task-lifecycle'])
  })
  it('uses the actual owner state once the model is active', async () => {
    active = true
    mount()
    await screen.findByText('Разработка')
    expect(screen.getByText('Готова к работе')).toBeInTheDocument()
    expect(screen.queryByText('Прежнее поле статуса')).not.toBeInTheDocument()
  })
  it('never exposes a legacy status writer when activation cannot be checked', async () => {
    unavailable = true
    mount()
    await screen.findByText('Не удалось проверить модель задач')
    expect(screen.queryByText('Прежнее поле статуса')).not.toBeInTheDocument()
    expect(requested.every((path) => path === '/tracker/api/v1/task-lifecycle')).toBe(true)
  })
})

it('keeps a read-only issue read-only even when a cached state contains actions', () => {
  const client = new QueryClient()
  render(
    <QueryClientProvider client={client}>
      <TaskLifecycleActions
        disabled
        lifecycle={{
          task_id: 'task',
          state: { stage: 'Development', status: 'Ready', revision: 9 },
          execution_busy: false,
          allowed_actions: [
            {
              action: 'start',
              target: { stage: 'Development', status: 'InProgress', revision: 10 },
            },
          ],
        }}
      />
    </QueryClientProvider>,
  )
  const start = screen.getByRole('button', { name: 'Начать этап' })
  expect(start).toBeDisabled()
  fireEvent.click(start)
  expect(wire).not.toHaveBeenCalled()
})
