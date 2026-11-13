import { cleanup, render, screen } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { ThemeProvider } from '@pdlc/ui/lib'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { getSdlcLifecycle, SdlcRequestError, type SdlcContext } from '@/api/sdlc'
import { lifecycleContext, lifecycleReadback, resultId } from '@/api/test-fixtures/lifecycle'
import i18n from '@/shared/i18n/config'
import { SdlcExecution } from './SdlcExecution'

vi.mock('@/api/sdlc', async (original) => ({
  ...(await original<typeof import('@/api/sdlc')>()),
  getSdlcLifecycle: vi.fn(),
}))
function mount(context = lifecycleContext, enabled = true) {
  return render(
    <ThemeProvider>
      <QueryClientProvider client={new QueryClient()}>
        <SdlcExecution
          context={context}
          queryKey={['sdlc', context.task_id, 'human']}
          enabled={enabled}
        />
      </QueryClientProvider>
    </ThemeProvider>,
  )
}
describe('current execution ownership', () => {
  beforeEach(async () => {
    vi.clearAllMocks()
    await i18n.changeLanguage('en')
    vi.mocked(getSdlcLifecycle).mockResolvedValue(lifecycleReadback())
  })
  afterEach(cleanup)
  it('does not infer a running agent from a queued task or a retained PM assignment', async () => {
    mount({
      ...lifecycleContext,
      assignment: {
        assignment_id: resultId,
        execution_id: resultId,
        agent_id: resultId,
        version: 4,
        machine_subject: 'retained-pm',
      },
    })
    expect(await screen.findByText('Queued; execution has not started')).toBeVisible()
    expect(screen.getByText('No current assignee is confirmed')).toBeVisible()
  })
  it.each(['Draft', 'Clarification', 'Backlog'] as const)(
    'does not read delivery storage during %s preparation',
    (stage) => {
      mount({ ...lifecycleContext, stage, status: 'Ready' })
      expect(getSdlcLifecycle).not.toHaveBeenCalled()
      expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    },
  )
  it('does not read execution before the context publishes a status', () => {
    mount({ ...lifecycleContext, status: null })
    expect(
      screen.getByText('Execution details have not been published for this task.'),
    ).toBeVisible()
    expect(getSdlcLifecycle).not.toHaveBeenCalled()
  })
  it('blocks reads while task context is stale or unavailable', () => {
    mount(lifecycleContext, false)
    expect(screen.getByText('Refresh task state before reading execution details.')).toBeVisible()
    expect(getSdlcLifecycle).not.toHaveBeenCalled()
  })
  it.each([403, 404, 503])(
    'shows read failure %s without inventing a successful state',
    async (status) => {
      vi.mocked(getSdlcLifecycle).mockRejectedValue(new SdlcRequestError(status))
      mount()
      expect(await screen.findByRole('alert')).toBeVisible()
      expect(screen.queryByText('Queued; execution has not started')).not.toBeInTheDocument()
    },
  )
  it('uses only the server runtime state when execution is running', async () => {
    const context: SdlcContext = { ...lifecycleContext, status: 'InProgress' }
    const data = lifecycleReadback()
    data.runtime_state = 'running'
    data.receipt.task.runtime = 'running'
    data.receipt.task.status = 'InProgress'
    vi.mocked(getSdlcLifecycle).mockResolvedValue(data)
    mount(context)
    expect(await screen.findByText('Running')).toBeVisible()
    expect(screen.queryByText('Queued; execution has not started')).not.toBeInTheDocument()
  })
})
