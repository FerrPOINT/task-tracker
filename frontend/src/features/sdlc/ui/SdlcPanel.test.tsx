import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { ThemeProvider } from '@sdlc/ui/lib'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  confirmRequirements,
  getSdlcMetadata,
  getSdlcSnapshot,
  SdlcRequestError,
  type AnalysisIntent,
  type Confirmation,
  type MetadataEvent,
  type SdlcSnapshot,
} from '@/api/sdlc'
import { useAuthStore } from '@/shared/auth/store'
import i18n from '@/shared/i18n/config'
import { SdlcPanel } from './SdlcPanel'

vi.mock('@/api/sdlc', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/api/sdlc')>()),
  getSdlcSnapshot: vi.fn(),
  getSdlcMetadata: vi.fn(),
  confirmRequirements: vi.fn(),
}))

const task = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'
const confirmationId = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'
const intentId = 'cccccccc-cccc-4ccc-8ccc-cccccccccccc'
const hash = 'a'.repeat(64)
const created = '2026-10-03T10:00:00Z'
const initial: SdlcSnapshot = {
  context: {
    contract_version: 1,
    tracker_instance_id: 'tracker-test',
    project_id: task,
    task_id: task,
    root_task_id: task,
    owner_subject: 'owner',
    stage: 'Clarification',
    requirement_revision: 2,
    waiting_reason: 'waiting_for_owner_confirmation',
    permissions: { can_answer: false, can_confirm: true },
    assignment: null,
  },
  requirements: {
    revision: 2,
    content_hash: hash,
    author_subject: 'pm',
    created_at: created,
    goal: 'Exact goal shown to the owner',
    scope: ['Business-only analysis'],
    exclusions: [],
    scenarios: ['Scenario'],
    acceptance_criteria: ['Criterion'],
    constraints: [],
    dependencies: [],
    assumptions: [],
    checklist: ['check'],
    prerequisites: ['prerequisite'],
  },
  intent: null,
}
const intent: AnalysisIntent = {
  contract_version: 1,
  tracker_instance_id: 'tracker-test',
  project_id: task,
  task_id: task,
  root_task_id: task,
  intent_id: intentId,
  confirmation_id: confirmationId,
  requirement_revision: 2,
  content_hash: hash,
  stage: 'Analysis',
  status: 'Ready',
  role: 'Analyst',
  workflow: 'hermes-sdlc:analyst',
  mode: 'analysis',
  scope: 'business',
  cycle: 0,
  attempt: 0,
  operation_key: `analysis:${confirmationId}`,
  created_at: created,
}
const queued: SdlcSnapshot = {
  ...initial,
  context: {
    ...initial.context,
    stage: 'Analysis',
    waiting_reason: 'queued_for_analysis',
    permissions: { can_answer: false, can_confirm: false },
  },
  intent,
}
const receipt: Confirmation = {
  id: confirmationId,
  task_id: task,
  revision: 2,
  content_hash: hash,
  owner_subject: 'owner',
  created_at: created,
  stage: 'Backlog',
}
const event: MetadataEvent = {
  sequence: '19',
  event_id: intentId,
  task_id: task,
  event_type: 'analysis.intent_created',
  created_at: created,
  metadata_sha256: 'b'.repeat(64),
  payload: {
    tracker_instance_id: 'tracker-test',
    project_id: task,
    root_task_id: task,
    owner_subject: 'owner',
    stage: 'Analysis',
    current_requirement_revision: 2,
    resource: intent,
  },
}

function mount(issueId = task) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  const result = render(
    <ThemeProvider>
      <QueryClientProvider client={client}>
        <SdlcPanel key={issueId} issueId={issueId} />
      </QueryClientProvider>
    </ThemeProvider>,
  )
  return { ...result, client }
}

describe('SDLC owner panel (isolated controller fixtures, not live admission)', () => {
  beforeEach(async () => {
    vi.clearAllMocks()
    await i18n.changeLanguage('en')
    useAuthStore.setState({ token: 'isolated-test-token', userId: 'owner' })
    vi.mocked(getSdlcSnapshot).mockResolvedValue(structuredClone(initial))
    vi.mocked(getSdlcMetadata).mockImplementation(async (_id, after) => ({
      contract_version: 1,
      projection: 'metadata_v1',
      after,
      next_after: after,
      has_more: false,
      events: [],
    }))
  })
  afterEach(() => {
    cleanup()
    useAuthStore.getState().logout()
  })

  it('shows loading and does not fabricate a queue or confirmation permission', () => {
    vi.mocked(getSdlcSnapshot).mockReturnValue(new Promise(() => {}))
    mount()
    expect(screen.getByRole('status', { name: 'Loading SDLC state' })).toBeInTheDocument()
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
    expect(screen.queryByText(/Analysis \/ Ready/)).not.toBeInTheDocument()
  })
  it('does not request owner resources without a credential', () => {
    useAuthStore.getState().logout()
    mount()
    expect(screen.getByRole('alert')).toBeInTheDocument()
    expect(getSdlcSnapshot).not.toHaveBeenCalled()
    expect(getSdlcMetadata).not.toHaveBeenCalled()
    expect(confirmRequirements).not.toHaveBeenCalled()
  })
  it('distinguishes missing context from Draft without requirements', async () => {
    vi.mocked(getSdlcSnapshot).mockResolvedValue(null)
    const view = mount()
    expect(await screen.findByText('No SDLC context')).toBeInTheDocument()
    expect(getSdlcMetadata).not.toHaveBeenCalled()
    view.unmount()
    vi.mocked(getSdlcSnapshot).mockResolvedValue({
      ...initial,
      requirements: null,
      context: {
        ...initial.context,
        stage: 'Draft',
        requirement_revision: null,
        permissions: { can_answer: false, can_confirm: false },
      },
    })
    mount()
    expect(await screen.findByText('Requirements have not been published')).toBeInTheDocument()
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
  })
  it.each([401, 403, 409, 503])(
    'shows HTTP %i as an error, not empty or success',
    async (status) => {
      vi.mocked(getSdlcSnapshot).mockRejectedValue(new SdlcRequestError(status))
      mount()
      expect(await screen.findByRole('alert')).toBeInTheDocument()
      expect(screen.queryByText('No SDLC context')).not.toBeInTheDocument()
      expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
    },
  )
  it('requires exact owner acknowledgement and shows Analysis only after intent readback', async () => {
    let finish!: (value: Confirmation) => void
    vi.mocked(confirmRequirements).mockReturnValue(
      new Promise((resolve) => {
        finish = resolve
      }),
    )
    mount()
    const button = await screen.findByRole('button', { name: 'Confirm requirements' })
    expect(button).toBeDisabled()
    expect(screen.getByText(hash)).toBeInTheDocument()
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
    )
    fireEvent.click(button)
    await waitFor(() => expect(confirmRequirements).toHaveBeenCalledOnce())
    expect(confirmRequirements).toHaveBeenCalledWith(task, 2, {
      content_hash: hash,
      idempotency_key: expect.any(String),
    })
    expect(screen.getByRole('button', { name: 'Confirming requirements' })).toBeDisabled()
    expect(screen.queryByText(/Analysis \/ Ready/)).not.toBeInTheDocument()
    vi.mocked(getSdlcSnapshot).mockResolvedValue(queued)
    await act(async () => {
      finish(receipt)
    })
    expect(
      await screen.findByText('Analysis / Ready · Queued, awaiting admission'),
    ).toBeInTheDocument()
    expect(screen.getByText(intentId)).toBeInTheDocument()
    expect(screen.getByText(`analysis:${confirmationId}`)).toBeInTheDocument()
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
    expect(screen.queryByText(/running|InProgress|Completed/)).not.toBeInTheDocument()
  })
  it('does not substitute a Backlog publication receipt for queued intent readback', async () => {
    vi.mocked(confirmRequirements).mockResolvedValue(receipt)
    mount()
    fireEvent.click(await screen.findByRole('checkbox'))
    fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
    expect(
      await screen.findByText('Requirements confirmation saved; refreshing queue state'),
    ).toBeInTheDocument()
    expect(screen.queryByText(/Analysis \/ Ready/)).not.toBeInTheDocument()
  })
  it('reuses the confirmation operation key after an unknown POST result', async () => {
    vi.mocked(confirmRequirements).mockRejectedValue(new SdlcRequestError(503))
    mount()
    fireEvent.click(await screen.findByRole('checkbox'))
    fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
    await screen.findByText('SDLC or Central Auth is unavailable')
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeEnabled(),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
    await waitFor(() => expect(confirmRequirements).toHaveBeenCalledTimes(2))
    expect(vi.mocked(confirmRequirements).mock.calls[0]).toEqual(
      vi.mocked(confirmRequirements).mock.calls[1],
    )
  })
  it('requires a new acknowledgement when refresh changes the revision or pin', async () => {
    mount()
    fireEvent.click(await screen.findByRole('checkbox'))
    vi.mocked(getSdlcSnapshot).mockResolvedValue({
      ...initial,
      context: { ...initial.context, requirement_revision: 3 },
      requirements: { ...initial.requirements!, revision: 3, content_hash: 'b'.repeat(64) },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Refresh SDLC state' }))
    const acknowledgement = await screen.findByRole('checkbox', {
      name: 'I confirm the requirements of revision 3',
    })
    expect(acknowledgement).not.toBeChecked()
    expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
    expect(confirmRequirements).not.toHaveBeenCalled()
  })
  it('does not retry a stale confirmation against a new revision automatically', async () => {
    vi.mocked(confirmRequirements).mockRejectedValue(new SdlcRequestError(409))
    mount()
    fireEvent.click(await screen.findByRole('checkbox'))
    vi.mocked(getSdlcSnapshot).mockResolvedValue({
      ...initial,
      context: { ...initial.context, requirement_revision: 3 },
      requirements: { ...initial.requirements!, revision: 3, content_hash: 'b'.repeat(64) },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
    expect(
      await screen.findByRole('checkbox', { name: 'I confirm the requirements of revision 3' }),
    ).not.toBeChecked()
    expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
    expect(confirmRequirements).toHaveBeenCalledOnce()
    expect(vi.mocked(confirmRequirements).mock.calls[0]?.[1]).toBe(2)
    expect(screen.queryByText(/Analysis \/ Ready/)).not.toBeInTheDocument()
  })
  it('keeps saved data visibly stale on refresh failure and disables confirmation', async () => {
    mount()
    fireEvent.click(await screen.findByRole('checkbox'))
    vi.mocked(getSdlcSnapshot).mockRejectedValue(new SdlcRequestError(503))
    fireEvent.click(screen.getByRole('button', { name: 'Refresh SDLC state' }))
    expect(await screen.findByText(/Saved data · refresh failed/)).toBeInTheDocument()
    expect(screen.getByText('Exact goal shown to the owner')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
  })
  it('removes cached private pins when fresh ACL denies access', async () => {
    vi.mocked(getSdlcSnapshot).mockResolvedValue(queued)
    mount()
    await screen.findByText(intentId)
    vi.mocked(getSdlcSnapshot).mockRejectedValue(new SdlcRequestError(403))
    fireEvent.click(screen.getByRole('button', { name: 'Refresh SDLC state' }))
    expect(await screen.findByText('Access to this SDLC resource is denied')).toBeInTheDocument()
    expect(screen.queryByText(intentId)).not.toBeInTheDocument()
    expect(screen.queryByText(hash)).not.toBeInTheDocument()
  })
  it('does not reuse another credential data or owner acknowledgement', async () => {
    mount()
    fireEvent.click(await screen.findByRole('checkbox'))
    vi.mocked(getSdlcSnapshot).mockRejectedValue(new SdlcRequestError(403))
    act(() => useAuthStore.setState({ token: 'different-isolated-token', userId: 'foreign' }))
    await screen.findByText('Access to this SDLC resource is denied')
    expect(screen.queryByText('Exact goal shown to the owner')).not.toBeInTheDocument()
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
  })
  it('renders the typed intent event as queued history and pages only by the server cursor', async () => {
    vi.mocked(getSdlcSnapshot).mockResolvedValue(queued)
    vi.mocked(getSdlcMetadata).mockImplementation(async (_id, after) => ({
      contract_version: 1,
      projection: 'metadata_v1',
      after,
      next_after: '19',
      has_more: after === '0',
      events: after === '0' ? [event] : [],
    }))
    mount()
    const history = await screen.findByRole('region', { name: 'SDLC events' })
    expect(await within(history).findByText('Analysis intent queued')).toBeInTheDocument()
    fireEvent.click(within(history).getByRole('button', { name: 'Next events' }))
    await waitFor(() => expect(getSdlcMetadata).toHaveBeenCalledWith(task, '19', expect.anything()))
    expect(await within(history).findByText('No events after this sequence')).toBeInTheDocument()
  })
  it('does not render a failed metadata refresh as an empty or current history', async () => {
    vi.mocked(getSdlcSnapshot).mockResolvedValue(queued)
    vi.mocked(getSdlcMetadata).mockResolvedValue({
      contract_version: 1,
      projection: 'metadata_v1',
      after: '0',
      next_after: '19',
      has_more: false,
      events: [event],
    })
    mount()
    const history = await screen.findByRole('region', { name: 'SDLC events' })
    await within(history).findByText('Analysis intent queued')
    vi.mocked(getSdlcMetadata).mockRejectedValue(new SdlcRequestError(503))
    fireEvent.click(screen.getByRole('button', { name: 'Refresh SDLC state' }))
    expect(
      await within(history).findByText('SDLC or Central Auth is unavailable'),
    ).toBeInTheDocument()
    expect(within(history).getByText(/Saved data · refresh failed/)).toBeInTheDocument()
    expect(within(history).queryByText('No events after this sequence')).not.toBeInTheDocument()
    expect(within(history).queryByText('Analysis intent queued')).not.toBeInTheDocument()
  })
  it('renders reservation history as prepared and still awaiting admission', async () => {
    vi.mocked(getSdlcSnapshot).mockResolvedValue(queued)
    const reserved: MetadataEvent = {
      ...event,
      event_type: 'analysis.assignment_reserved',
      payload: {
        ...event.payload,
        resource: {
          assignment_id: confirmationId,
          execution_id: intentId,
          workflow_task_ref: 'SDLC-3',
          intent_id: intentId,
          routing_snapshot_id: task,
          agent_id: task,
          fencing_token: 1,
          assignment_hash: hash,
        },
      },
    }
    vi.mocked(getSdlcMetadata).mockResolvedValue({
      contract_version: 1,
      projection: 'metadata_v1',
      after: '0',
      next_after: '19',
      has_more: false,
      events: [reserved],
    })
    mount()
    const history = await screen.findByRole('region', { name: 'SDLC events' })
    expect(
      await within(history).findByText('Analysis assignment prepared, awaiting admission'),
    ).toBeInTheDocument()
    expect(screen.queryByText(/InProgress|dispatch accepted|run started/)).not.toBeInTheDocument()
  })
  it('keeps owner permissions server-derived and missing intent unverified', async () => {
    vi.mocked(getSdlcSnapshot).mockResolvedValue({
      ...initial,
      context: { ...initial.context, permissions: { can_answer: false, can_confirm: false } },
    })
    const view = mount()
    expect(await screen.findByText('Owner confirmation is unavailable')).toBeInTheDocument()
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
    view.unmount()
    vi.mocked(getSdlcSnapshot).mockResolvedValue({ ...queued, intent: null })
    mount()
    expect(
      await screen.findByText('Analysis intent was not found; queue state is not verified'),
    ).toBeInTheDocument()
    expect(screen.queryByText(/Analysis \/ Ready/)).not.toBeInTheDocument()
  })
})
