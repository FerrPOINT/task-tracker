import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, render, screen, fireEvent, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
const wire = vi.hoisted(() => {
  vi.stubEnv('VITE_API_BASE_URL', 'https://tracker.example.test/tracker')
  const fn = vi.fn()
  vi.stubGlobal('fetch', fn)
  return fn
})
import { TaskHandoffAction } from './TaskHandoffAction'
import { useAuthStore } from '@/shared/auth/store'
import type { HandoffPreview, HandoffOperation, HandoffCommand } from '@/api/task-handoff'
let view: HandoffPreview
let posts: HandoffCommand[]
let lost = false
function operation(command: HandoffCommand): HandoffOperation {
  return {
    execution_requested: true,
    cancellation: null,
    receipt: {
      contract_version: 1,
      handoff_id: 'handoff',
      task_id: 'task',
      project_id: 'project',
      actor_subject: 'owner',
      entry: {
        state: { stage: 'Draft', status: 'Ready', revision: 3 },
        role: 'project_manager',
        mode: 'draft',
      },
      confirmed_issue_sha256: command.confirmed_issue_sha256,
      issue_materials_sha256: 'b'.repeat(64),
      resulting_revision: 4,
      requirement_revision: 1,
      requirements_sha256: 'c'.repeat(64),
      confirmation_id: 'confirm',
      routing_snapshot_id: 'route',
      source_binding_id: 'source',
      idempotency_key: command.idempotency_key,
      created_at: '2026-10-31T00:00:00Z',
    },
  }
}
beforeEach(() => {
  useAuthStore.setState({ token: 'owner-token' })
  posts = []
  lost = false
  view = {
    state: { stage: 'Draft', status: 'Ready', revision: 3 },
    issue_sha256: 'a'.repeat(64),
    routing_policy_version: 7,
    current_request: null,
    required_applications: ['task-tracker', 'fleet-control', 'project-workflow', 'ci-cd'],
    blockers: [],
  }
  wire.mockReset()
  wire.mockImplementation(async (request: Request) => {
    const url = new URL(request.url)
    if (request.method === 'GET' && url.pathname.endsWith('/handoff')) return Response.json(view)
    if (request.method === 'POST' && url.pathname.endsWith('/handoff')) {
      const command = (await request.json()) as HandoffCommand
      posts.push(command)
      if (lost) throw new TypeError('Response lost')
      const op = operation(command)
      view = {
        ...view,
        blockers: ['execution_busy'],
        current_request: { operation: op, core_revision: 4, can_cancel: true },
      }
      return Response.json(op, { status: 202 })
    }
    if (request.method === 'GET' && url.pathname.includes('/operations/'))
      return new Response(null, { status: 404 })
    if (request.method === 'POST' && url.pathname.endsWith('/cancel')) {
      const command = await request.json()
      expect(command.expected_revision).toBe(4)
      view = { ...view, current_request: null, blockers: [], state: { ...view.state, revision: 5 } }
      return Response.json({
        task_id: 'task',
        handoff_id: 'handoff',
        actor_subject: 'owner',
        resulting_revision: 5,
        created_at: '2026-10-31T00:01:00Z',
      })
    }
    throw new Error('Unexpected request ' + request.method + ' ' + url.pathname)
  })
})
afterEach(() => act(() => useAuthStore.getState().logout()))
function mount(disabled = false) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  render(
    <QueryClientProvider client={client}>
      <TaskHandoffAction issueId="task" summary="Реальная задача" disabled={disabled} />
    </QueryClientProvider>,
  )
  return client
}
async function open() {
  fireEvent.click(screen.getByRole('button', { name: 'Работа с агентом' }))
  return screen.findByRole('dialog', { name: 'Передать текущий этап агенту' })
}
async function consent() {
  const control = await screen.findByRole('checkbox', {
    name: 'Подтверждаю материалы задачи и передачу текущего этапа агенту.',
  })
  await waitFor(() => expect(control).toBeEnabled())
  fireEvent.click(control)
}
describe('explicit human current-stage request', () => {
  it('does not fetch or launch until opened and requires explicit material consent', async () => {
    mount()
    expect(wire).not.toHaveBeenCalled()
    await open()
    await screen.findByRole('textbox', { name: /^Цель/ })
    expect(screen.getByRole('button', { name: 'Передать агенту' })).toBeDisabled()
    expect(posts).toEqual([])
    await consent()
    fireEvent.click(screen.getByRole('button', { name: 'Передать агенту' }))
    await screen.findByText('Запрос сохранён и ожидает назначения. Стадия задачи сохраняется.')
    expect(posts).toHaveLength(1)
    expect(posts[0]!.requirements.goal).toBe('Реальная задача')
    expect(posts[0]!.requirements.checklist).toEqual([])
    expect(posts[0]!.expected_revision).toBe(3)
  })
  it('leaves manual work available when execution is not configured', async () => {
    view.blockers = ['execution_not_configured']
    mount()
    await open()
    await screen.findByText(
      'Исполнение агентами для проекта не настроено. Задачу можно вести вручную.',
    )
    expect(screen.getByRole('button', { name: 'Передать агенту' })).toBeDisabled()
    expect(posts).toEqual([])
  })
  it('invalidates consent after a material or routing update without submitting stale data', async () => {
    const client = mount()
    await open()
    await consent()
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Передать агенту' })).toBeEnabled(),
    )
    view = { ...view, issue_sha256: 'd'.repeat(64), routing_policy_version: 8 }
    await act(async () => {
      await client.invalidateQueries({ queryKey: ['task-handoff', 'task'] })
    })
    await waitFor(() => expect(screen.getByRole('checkbox')).not.toBeChecked())
    expect(screen.getByRole('button', { name: 'Передать агенту' })).toBeDisabled()
    expect(posts).toEqual([])
  })
  it('keeps an unknown outcome on the original key and never resends automatically', async () => {
    lost = true
    mount()
    await open()
    await consent()
    fireEvent.click(screen.getByRole('button', { name: 'Передать агенту' }))
    await screen.findByRole('button', { name: 'Проверить результат' })
    expect(posts).toHaveLength(1)
    fireEvent.click(screen.getByRole('button', { name: 'Проверить результат' }))
    await screen.findByText(
      'Исходная операция пока не найдена. Можно проверить ещё раз или повторить тот же запрос.',
    )
    expect(posts).toHaveLength(1)
    fireEvent.click(screen.getByRole('button', { name: 'Повторить исходный запрос' }))
    await waitFor(() => expect(posts).toHaveLength(2))
    expect(posts[1]).toEqual(posts[0])
  })
  it('shows and cancels an already queued request even without launch applications', async () => {
    const command = {
      expected_revision: 3,
      expected_routing_policy_version: 7,
      confirmed_issue_sha256: 'a'.repeat(64),
      idempotency_key: 'original',
      requirements: {
        goal: 'Already saved',
        scope: [],
        exclusions: [],
        scenarios: [],
        acceptance_criteria: [],
        constraints: [],
        dependencies: [],
        assumptions: [],
        checklist: [],
        prerequisites: [],
      },
    }
    view = {
      ...view,
      blockers: ['application_access_denied'],
      current_request: { operation: operation(command), core_revision: 4, can_cancel: true },
    }
    mount()
    await open()
    const cancel = await screen.findByRole('button', { name: 'Отменить запрос' })
    fireEvent.click(cancel)
    await screen.findByRole('textbox', { name: /^Цель/ })
    expect(posts).toEqual([])
  })
  it('cannot cancel a read-only queue or submit from a read-only issue', async () => {
    mount(true)
    await open()
    await screen.findByRole('textbox', { name: /^Цель/ })
    expect(screen.getByRole('button', { name: 'Передать агенту' })).toBeDisabled()
    expect(posts).toEqual([])
  })
  it('retires the previous identity form and confirmation on credential change', async () => {
    mount()
    await open()
    await consent()
    await act(async () => useAuthStore.setState({ token: 'successor-token' }))
    await waitFor(() => expect(screen.getByRole('checkbox')).not.toBeChecked())
    expect(screen.getByRole('button', { name: 'Передать агенту' })).toBeDisabled()
    expect(posts).toEqual([])
  })
})
