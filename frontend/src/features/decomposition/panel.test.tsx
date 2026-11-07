import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { DecompositionPanel } from './panel'
import {
  readPlan,
  savePlan,
  readOperation,
  readHistory,
  readRevision,
  PlanRequestError,
  type PlanRevision,
} from '@/api/decomposition'
import { useAuthStore } from '@/shared/auth/store'

vi.mock('@/api/decomposition', async (original) => ({
  ...(await original<typeof import('@/api/decomposition')>()),
  readPlan: vi.fn(),
  savePlan: vi.fn(),
  readOperation: vi.fn(),
  readHistory: vi.fn(),
  readRevision: vi.fn(),
}))
const plan: PlanRevision = {
  task_id: 'issue',
  revision: 1,
  author_name: 'Architect',
  author_subject: 'architect',
  reason: '',
  created_at: '2026-11-03T10:00:00Z',
  content_sha256: 'a'.repeat(64),
  content: {
    overview: 'Existing profile and API',
    source: null,
    sections: [
      {
        id: '11111111-1111-4111-8111-111111111111',
        title: 'Storage',
        description: 'Persist preferences',
        criteria: 'Read after write and deny foreign users',
        requirements: 'R1, R2',
      },
      {
        id: '22222222-2222-4222-8222-222222222222',
        title: 'Form',
        description: 'Use existing form',
        criteria: 'Preserve selection after failure',
        requirements: 'R1',
      },
    ],
  },
}
const readback = (value: PlanRevision | null = plan) => ({
  plan: value,
  can_edit: true,
  edit_restriction: null,
  current_source: null,
})
function setup() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  const router = createMemoryRouter(
    [
      {
        path: '*',
        element: (
          <QueryClientProvider client={client}>
            <DecompositionPanel issueId="issue" />
          </QueryClientProvider>
        ),
      },
    ],
    { initialEntries: ['/issues/issue?tab=decomposition'] },
  )
  render(<RouterProvider router={router} />)
  return { user: userEvent.setup(), router, client }
}
async function edit(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('button', { name: 'Редактировать' }))
  await user.type(screen.getByLabelText(/Причина изменения/), 'Clarify boundaries')
}
beforeEach(() => {
  vi.resetAllMocks()
  useAuthStore.setState({ token: 'session-a', userId: 'human' })
  vi.mocked(readPlan).mockResolvedValue(readback())
})
afterEach(cleanup)
describe('persistent task decomposition', () => {
  it('saves actual content with CAS, stable IDs and reason; opens history read-only', async () => {
    vi.mocked(savePlan).mockImplementation(async (_id, command) => ({
      changed: true,
      request_sha256: 'b'.repeat(64),
      result: { ...plan, revision: 2, content: command.content },
    }))
    vi.mocked(readHistory).mockResolvedValue({ revisions: [{ ...plan }], next_before: null })
    vi.mocked(readRevision).mockResolvedValue(plan)
    const { user } = setup()
    await edit(user)
    await user.click(screen.getByRole('button', { name: 'Переместить Form вверх' }))
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    await screen.findByText('Сохранена редакция 2.')
    expect(savePlan).toHaveBeenCalledWith(
      'issue',
      expect.objectContaining({
        expected_revision: 1,
        reason: 'Clarify boundaries',
        idempotency_key: expect.any(String),
        content: expect.objectContaining({
          sections: [plan.content.sections[1], plan.content.sections[0]],
        }),
      }),
    )
    await user.click(screen.getByRole('button', { name: /Редакция 1 — история/ }))
    expect(await screen.findByRole('dialog')).toHaveTextContent('Редакции плана')
    await waitFor(() =>
      expect(readRevision).toHaveBeenCalledWith('issue', 1, expect.any(AbortSignal)),
    )
    expect(screen.queryByRole('form')).not.toBeInTheDocument()
  })
  it('keeps draft through tabs and cancel; direct links disclose only their stable ID', async () => {
    const { user, router } = setup()
    await screen.findByRole('button', { name: 'Редактировать' })
    expect(document.querySelectorAll('details[open]')).toHaveLength(0)
    await user.click(screen.getByRole('link', { name: 'Ссылка на раздел «Form»' }))
    await waitFor(() =>
      expect(document.getElementById(`section-${plan.content.sections[1]!.id}`)).toHaveAttribute(
        'open',
      ),
    )
    expect(document.getElementById(`section-${plan.content.sections[0]!.id}`)).not.toHaveAttribute(
      'open',
    )
    await edit(user)
    await act(() => router.navigate('/issues/issue?tab=comments'))
    expect(screen.getByLabelText(/Причина изменения/)).toHaveValue('Clarify boundaries')
    await user.click(screen.getByRole('button', { name: 'Отмена' }))
    expect(savePlan).not.toHaveBeenCalled()
  })
  it('preserves a newer draft while reconciling a lost response with the original key', async () => {
    vi.mocked(savePlan).mockRejectedValueOnce(new TypeError('network lost'))
    vi.mocked(readOperation).mockResolvedValue({
      changed: true,
      request_sha256: 'b'.repeat(64),
      result: { ...plan, revision: 2 },
    })
    const { user } = setup()
    await edit(user)
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    await screen.findByText(/Результат сохранения пока неизвестен/)
    await user.type(screen.getByLabelText(/Архитектурное решение/), ' NEW INPUT')
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    await screen.findByText(/Новый черновик остался/)
    expect(screen.getByLabelText(/Архитектурное решение/)).toHaveValue(
      'Existing profile and API NEW INPUT',
    )
    expect(savePlan).toHaveBeenCalledTimes(1)
    expect(readOperation).toHaveBeenCalledWith(
      'issue',
      vi.mocked(savePlan).mock.calls[0]![1].idempotency_key,
    )
  })
  it('shows remote sections separately on conflict and requires explicit continuation', async () => {
    const remote = {
      ...plan,
      revision: 2,
      content: { ...plan.content, overview: 'Remote architecture' },
    }
    vi.mocked(savePlan).mockRejectedValueOnce(new PlanRequestError(409))
    const { user } = setup()
    await edit(user)
    vi.mocked(readPlan).mockResolvedValue(readback(remote))
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    await screen.findByText('План или условия редактирования изменились')
    expect(screen.getByLabelText(/Архитектурное решение/)).toHaveValue(plan.content.overview)
    expect(screen.getByRole('button', { name: 'Сохранить план' })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: /Я сравнил изменения/ }))
    expect(screen.getByRole('button', { name: 'Сохранить план' })).toBeEnabled()
    expect(savePlan).toHaveBeenCalledTimes(1)
  })
  it('hides plan and draft after revoked access', async () => {
    vi.mocked(savePlan).mockRejectedValueOnce(new PlanRequestError(403))
    const { user } = setup()
    await edit(user)
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    await screen.findByText(/Доступ к плану недоступен/)
    expect(screen.queryByRole('form')).not.toBeInTheDocument()
    expect(screen.queryByText('Storage')).not.toBeInTheDocument()
  })
  it('also closes the plan when access is revoked while reloading a conflict', async () => {
    vi.mocked(savePlan).mockRejectedValueOnce(new PlanRequestError(409))
    const { user } = setup()
    await edit(user)
    vi.mocked(readPlan).mockRejectedValue(new PlanRequestError(403))
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    await screen.findByText(/Доступ к плану недоступен/)
    expect(screen.queryByRole('form')).not.toBeInTheDocument()
  })
  it('retries an unaccepted uncertain operation with exactly its original body', async () => {
    vi.mocked(savePlan)
      .mockRejectedValueOnce(new TypeError('connection closed'))
      .mockImplementationOnce(async (_id, command) => ({
        changed: true,
        request_sha256: 'b'.repeat(64),
        result: { ...plan, revision: 2, content: command.content },
      }))
    vi.mocked(readOperation).mockRejectedValueOnce(new PlanRequestError(404))
    const { user } = setup()
    await edit(user)
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    await screen.findByText(/Результат сохранения пока неизвестен/)
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    await screen.findByText('Сохранена редакция 2.')
    expect(vi.mocked(savePlan).mock.calls[1]![1]).toEqual(vi.mocked(savePlan).mock.calls[0]![1])
  })
  it('does not let an old credential response erase the new session draft', async () => {
    let complete!: (value: Awaited<ReturnType<typeof savePlan>>) => void
    vi.mocked(savePlan).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve
        }),
    )
    const { user } = setup()
    await edit(user)
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    await act(() =>
      useAuthStore
        .getState()
        .setAuth({ token: 'session-b', userId: 'human', email: 'fixture@example.test' }),
    )
    await user.click(await screen.findByRole('button', { name: 'Редактировать' }))
    await user.type(screen.getByLabelText(/Причина изменения/), 'NEW SESSION DRAFT')
    await act(async () =>
      complete({ changed: true, request_sha256: 'b'.repeat(64), result: { ...plan, revision: 2 } }),
    )
    expect(screen.getByLabelText(/Причина изменения/)).toHaveValue('NEW SESSION DRAFT')
    expect(screen.queryByText('Сохранена редакция 2.')).not.toBeInTheDocument()
  })
  it('allows saving an unchanged plan without inventing a reason or a revision', async () => {
    vi.mocked(savePlan).mockResolvedValue({
      changed: false,
      request_sha256: 'b'.repeat(64),
      result: plan,
    })
    const { user } = setup()
    await user.click(await screen.findByRole('button', { name: 'Редактировать' }))
    expect(screen.getByLabelText(/Причина изменения/)).not.toBeRequired()
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    await screen.findByText('План не изменился. Редакция 1.')
    expect(savePlan).toHaveBeenCalledWith(
      'issue',
      expect.objectContaining({ expected_revision: 1, reason: '', content: plan.content }),
    )
  })
  it('keeps the leave decision available if a background refresh fails while editing', async () => {
    const { user, router, client } = setup()
    await edit(user)
    vi.mocked(readPlan).mockRejectedValue(new PlanRequestError(500))
    await act(async () => {
      await client.invalidateQueries({ queryKey: ['decomposition', 'issue'] })
    })
    await screen.findByText('Не удалось загрузить декомпозицию')
    await act(() => router.navigate('/board'))
    expect(await screen.findByRole('alertdialog', { name: 'Покинуть задачу?' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Покинуть без сохранения' }))
    expect(router.state.location.pathname).toBe('/board')
  })
})
