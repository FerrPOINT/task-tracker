import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { ThemeProvider } from '@pdlc/ui/lib'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  confirmRequirements,
  getSdlcMetadata,
  getSdlcSnapshot,
  getSdlcRoutingPolicy,
  SdlcRequestError,
  type AnalysisIntent,
  type Confirmation,
  type MetadataEvent,
  type SdlcSnapshot,
} from '@/api/sdlc'
import { useAuthStore } from '@/shared/auth/store'
import i18n from '@/shared/i18n/config'
import { SdlcPanel } from './SdlcPanel'
import { routingPolicy, singleTaskRoutingPolicy } from '@/api/test-fixtures/routing-policy'

vi.mock('@/api/sdlc', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/api/sdlc')>()),
  getSdlcSnapshot: vi.fn(),
  getSdlcMetadata: vi.fn(),
  getSdlcRoutingPolicy: vi.fn(),
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
    vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(null)
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

  it.each([
    ['en', 'Task source selected'],
    ['ru', 'Источник задачи выбран'],
  ])(
    'renders the source selection receipt in %s without starting or confirming work',
    async (language, label) => {
      await i18n.changeLanguage(language)
      vi.mocked(getSdlcSnapshot).mockResolvedValue({
        ...initial,
        context: {
          ...initial.context,
          stage: 'Draft',
          requirement_revision: null,
          waiting_reason: null,
          permissions: { can_answer: false, can_confirm: false },
        },
        requirements: null,
      })
      const selected: MetadataEvent = {
        ...event,
        event_type: 'source.selected',
        payload: {
          ...event.payload,
          stage: 'Draft',
          current_requirement_revision: null,
          resource: {
            enrollment_id: confirmationId,
            task_id: task,
            project_id: task,
            owner_subject: 'owner',
            source_revision: 1,
            policy_hash: hash,
            request_sha256: 'c'.repeat(64),
            source_hash: 'd'.repeat(64),
            created_at: created,
          },
        },
      }
      vi.mocked(getSdlcMetadata).mockResolvedValue({
        contract_version: 1,
        projection: 'metadata_v1',
        after: '0',
        next_after: selected.sequence,
        has_more: false,
        events: [selected],
      })
      mount()
      expect(await screen.findByText(label)).toBeInTheDocument()
      expect(screen.queryByText('source.selected')).not.toBeInTheDocument()
      expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
      expect(confirmRequirements).not.toHaveBeenCalled()
    },
  )
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
  it('leaves existing routing unchecked and preserves legacy omission', async () => {
    vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(routingPolicy(task))
    vi.mocked(confirmRequirements).mockResolvedValue(receipt)
    mount()
    const choice = await screen.findByRole('checkbox', {
      name: 'Publish with routing policy version 5',
    })
    expect(choice).not.toBeChecked()
    expect(screen.getByText(/Native readiness and dispatch are not granted/)).toBeInTheDocument()
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
    await waitFor(() => expect(confirmRequirements).toHaveBeenCalledOnce())
    expect(vi.mocked(confirmRequirements).mock.calls[0]?.[2]).not.toHaveProperty(
      'expected_routing_policy_version',
    )
  })
  it.each(['awaiting_confirmation', 'clarification', undefined, null] as const)(
    'fetches and submits v2 only with explicit consent in actual Draft %s',
    async (draft_submode) => {
      vi.mocked(getSdlcSnapshot).mockResolvedValue({
        ...initial,
        context: {
          ...initial.context,
          stage: 'Draft',
          status: 'InProgress',
          draft_submode,
          waiting_reason:
            draft_submode === 'clarification'
              ? 'waiting_for_owner_answers'
              : 'waiting_for_owner_confirmation',
          permissions: { can_answer: draft_submode === 'clarification', can_confirm: true },
        },
      })
      vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(singleTaskRoutingPolicy(task))
      vi.mocked(confirmRequirements).mockResolvedValue(receipt)
      mount()
      const choice = await screen.findByRole('checkbox', {
        name: 'Publish with routing policy version 5',
      })
      expect(choice).not.toBeChecked()
      expect(getSdlcRoutingPolicy).toHaveBeenCalledWith(
        expect.objectContaining({ stage: 'Draft', draft_submode }),
        expect.anything(),
      )
      expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
      fireEvent.click(choice)
      fireEvent.click(
        screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
      )
      fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
      await waitFor(() =>
        expect(confirmRequirements).toHaveBeenCalledWith(task, 2, {
          content_hash: hash,
          idempotency_key: expect.any(String),
          expected_routing_policy_version: 5,
        }),
      )
    },
  )
  it.each([
    ['brainstorming', true],
    ['clarification', false],
    ['awaiting_confirmation', false],
  ] as const)(
    'does not fetch or confirm Draft %s with can_confirm=%s',
    async (draft_submode, can_confirm) => {
      let completeMetadata!: (value: Awaited<ReturnType<typeof getSdlcMetadata>>) => void
      vi.mocked(getSdlcMetadata).mockReturnValueOnce(
        new Promise((resolve) => {
          completeMetadata = resolve
        }),
      )
      vi.mocked(getSdlcSnapshot).mockResolvedValue({
        ...initial,
        context: {
          ...initial.context,
          stage: 'Draft',
          draft_submode,
          permissions: { can_answer: draft_submode === 'clarification', can_confirm },
        },
      })
      mount()
      await screen.findByText('Exact goal shown to the owner')
      expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: 'Confirm requirements' })).not.toBeInTheDocument()
      expect(getSdlcRoutingPolicy).not.toHaveBeenCalled()
      await waitFor(() => expect(getSdlcMetadata).toHaveBeenCalledOnce())
      const refresh = screen.getByRole('button', { name: 'Refresh SDLC state' })
      // Snapshot content renders before metadata finishes; refresh is still disabled.
      expect(refresh).toBeDisabled()
      await act(async () => {
        completeMetadata({
          contract_version: 1,
          projection: 'metadata_v1',
          after: '0',
          next_after: '0',
          has_more: false,
          events: [],
        })
      })
      await waitFor(() => expect(refresh).toBeEnabled())
      fireEvent.click(refresh)
      await waitFor(() => expect(getSdlcSnapshot).toHaveBeenCalledTimes(2))
      await waitFor(() => expect(getSdlcMetadata).toHaveBeenCalledTimes(2))
      await waitFor(() => expect(refresh).toBeEnabled())
      expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
      expect(getSdlcRoutingPolicy).not.toHaveBeenCalled()
      expect(confirmRequirements).not.toHaveBeenCalled()
    },
  )
  it('refreshes policy in Draft awaiting_confirmation and blocks stale saved consent', async () => {
    const draft: SdlcSnapshot = {
      ...initial,
      context: {
        ...initial.context,
        stage: 'Draft',
        draft_submode: 'awaiting_confirmation',
      },
    }
    vi.mocked(getSdlcSnapshot).mockResolvedValue(draft)
    vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(singleTaskRoutingPolicy(task))
    mount()
    fireEvent.click(
      await screen.findByRole('checkbox', { name: 'Publish with routing policy version 5' }),
    )
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
    )
    vi.mocked(getSdlcSnapshot).mockRejectedValue(new SdlcRequestError(503))
    const policyReads = vi.mocked(getSdlcRoutingPolicy).mock.calls.length
    fireEvent.click(screen.getByRole('button', { name: 'Refresh SDLC state' }))
    await screen.findByText(/Saved data · refresh failed/)
    expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
    expect(
      screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
    ).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
    expect(confirmRequirements).not.toHaveBeenCalled()
    expect(getSdlcRoutingPolicy).toHaveBeenCalledTimes(policyReads)
    vi.mocked(getSdlcSnapshot).mockResolvedValue(draft)
    vi.mocked(getSdlcRoutingPolicy).mockResolvedValue({
      ...singleTaskRoutingPolicy(task),
      version: 6,
      routing_hash: 'b'.repeat(64),
    })
    fireEvent.click(screen.getByRole('button', { name: 'Refresh SDLC state' }))
    await screen.findByText(
      'The selected policy is not current or its source is unavailable. Refresh and explicitly review the choice.',
    )
    expect(
      screen.getByRole('checkbox', { name: 'Publish with routing policy version 5' }),
    ).toBeChecked()
    expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
    expect(confirmRequirements).not.toHaveBeenCalled()
  })
  it('removes Draft consent when a fresh read returns to brainstorming', async () => {
    vi.mocked(getSdlcSnapshot).mockResolvedValue({
      ...initial,
      context: {
        ...initial.context,
        stage: 'Draft',
        draft_submode: 'awaiting_confirmation',
      },
    })
    vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(singleTaskRoutingPolicy(task))
    mount()
    fireEvent.click(
      await screen.findByRole('checkbox', { name: 'Publish with routing policy version 5' }),
    )
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
    )
    vi.mocked(getSdlcSnapshot).mockResolvedValue({
      ...initial,
      context: {
        ...initial.context,
        stage: 'Draft',
        draft_submode: 'brainstorming',
      },
    })
    const policyReads = vi.mocked(getSdlcRoutingPolicy).mock.calls.length
    fireEvent.click(screen.getByRole('button', { name: 'Refresh SDLC state' }))
    await waitFor(() => expect(screen.queryByRole('checkbox')).not.toBeInTheDocument())
    expect(getSdlcRoutingPolicy).toHaveBeenCalledTimes(policyReads)
    expect(confirmRequirements).not.toHaveBeenCalled()
  })
  it.each([
    ['v1', routingPolicy, 'Package commit / manifest hash', 'Instruction profile / manifest hash'],
    [
      'v2',
      singleTaskRoutingPolicy,
      'Instruction profile / manifest hash',
      'Package commit / manifest hash',
    ],
  ] as const)(
    'shows only the actual %s source references without inventing package fields',
    async (_family, makePolicy, label, absentLabel) => {
      const policy = makePolicy(task)
      vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(policy)
      mount()
      const choice = await screen.findByRole('checkbox', {
        name: 'Publish with routing policy version 5',
      })
      expect(choice).not.toBeChecked()
      const publication = screen.getByRole('region', { name: 'SDLC publication' })
      fireEvent.click(within(publication).getByText('Seven-role policy references'))
      expect(within(publication).getAllByText(label)).toHaveLength(7)
      expect(within(publication).queryByText(absentLabel)).not.toBeInTheDocument()
      const route = policy.routes.analyst
      expect(
        within(publication).getByText(`${route.workflow_id} / ${route.workflow_key}`, {
          exact: false,
        }),
      ).toBeInTheDocument()
      expect(
        within(publication).getByText(/Native readiness and dispatch are not granted/),
      ).toBeInTheDocument()
      if (policy.contract_version === 2) {
        expect(
          within(publication).getAllByText(
            `single-task-v1 / ${policy.routes.analyst.instruction_manifest_sha256}`,
          ),
        ).toHaveLength(7)
        expect(
          within(publication).queryByText(/4b9b4c9297a13fb28a6ba2039af2f7cb719f2f58/),
        ).not.toBeInTheDocument()
      }
      expect(confirmRequirements).not.toHaveBeenCalled()
    },
  )
  it('publishes v2 only with explicit owner selection and the existing exact policy CAS', async () => {
    vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(singleTaskRoutingPolicy(task))
    vi.mocked(confirmRequirements).mockResolvedValue(receipt)
    mount()
    fireEvent.click(
      await screen.findByRole('checkbox', { name: 'Publish with routing policy version 5' }),
    )
    expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
    await waitFor(() =>
      expect(confirmRequirements).toHaveBeenCalledWith(task, 2, {
        content_hash: hash,
        idempotency_key: expect.any(String),
        expected_routing_policy_version: 5,
      }),
    )
  })
  it('shows a frozen profile Analysis intent as queued, not admitted or owner-confirmable', async () => {
    vi.mocked(getSdlcSnapshot).mockResolvedValue({
      ...queued,
      intent: { ...intent, workflow: 'single-task-v1:analyst' },
    })
    mount()
    expect(
      await screen.findByText('Analysis / Ready · Queued, awaiting admission'),
    ).toBeInTheDocument()
    expect(
      screen.getByText('Analyst / single-task-v1:analyst / analysis / business'),
    ).toBeInTheDocument()
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
    expect(getSdlcRoutingPolicy).not.toHaveBeenCalled()
    expect(confirmRequirements).not.toHaveBeenCalled()
  })
  it('posts only explicitly selected viewed policy version, without native admission', async () => {
    vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(routingPolicy(task))
    vi.mocked(confirmRequirements).mockResolvedValue(receipt)
    mount()
    fireEvent.click(
      await screen.findByRole('checkbox', { name: 'Publish with routing policy version 5' }),
    )
    expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
    await waitFor(() =>
      expect(confirmRequirements).toHaveBeenCalledWith(task, 2, {
        content_hash: hash,
        idempotency_key: expect.any(String),
        expected_routing_policy_version: 5,
      }),
    )
    expect(screen.queryByText(/native-ready|dispatch accepted/)).not.toBeInTheDocument()
  })
  it('retains stale selection and consent without silently selecting the new policy', async () => {
    vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(routingPolicy(task))
    mount()
    fireEvent.click(
      await screen.findByRole('checkbox', { name: 'Publish with routing policy version 5' }),
    )
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
    )
    vi.mocked(getSdlcRoutingPolicy).mockResolvedValue({
      ...routingPolicy(task),
      version: 6,
      routing_hash: 'f'.repeat(64),
    })
    fireEvent.click(screen.getByRole('button', { name: 'Refresh SDLC state' }))
    await screen.findByText(/The selected policy is not current/)
    expect(
      screen.getByRole('checkbox', { name: 'Publish with routing policy version 5' }),
    ).toBeChecked()
    expect(
      screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
    ).toBeChecked()
    expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
    expect(confirmRequirements).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('checkbox', { name: 'Publish with routing policy version 5' }))
    const current = screen.getByRole('checkbox', { name: 'Publish with routing policy version 6' })
    expect(current).not.toBeChecked()
    fireEvent.click(current)
    expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeEnabled()
  })
  it('blocks selected confirmation on a stale CAS while preserving both inputs', async () => {
    vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(routingPolicy(task))
    vi.mocked(confirmRequirements).mockRejectedValue(new SdlcRequestError(409))
    mount()
    fireEvent.click(
      await screen.findByRole('checkbox', { name: 'Publish with routing policy version 5' }),
    )
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
    await screen.findByText(/The selected policy is not current/)
    expect(
      screen.getByRole('checkbox', { name: 'Publish with routing policy version 5' }),
    ).toBeChecked()
    expect(
      screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
    ).toBeChecked()
    expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
    expect(confirmRequirements).toHaveBeenCalledOnce()
  })
  it('holds routed retry after unknown POST until fresh context read succeeds, preserving key/version', async () => {
    vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(routingPolicy(task))
    vi.mocked(confirmRequirements).mockRejectedValue(new SdlcRequestError(503))
    mount()
    fireEvent.click(
      await screen.findByRole('checkbox', { name: 'Publish with routing policy version 5' }),
    )
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
    )
    vi.mocked(getSdlcSnapshot).mockRejectedValue(new SdlcRequestError(503))
    fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
    await screen.findByText(/Saved data · refresh failed/)
    expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
    expect(
      screen.getByRole('checkbox', { name: 'Publish with routing policy version 5' }),
    ).toBeChecked()
    fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
    expect(confirmRequirements).toHaveBeenCalledOnce()
    vi.mocked(getSdlcSnapshot).mockResolvedValue(initial)
    fireEvent.click(screen.getByRole('button', { name: 'Refresh SDLC state' }))
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeEnabled(),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Confirm requirements' }))
    await waitFor(() => expect(confirmRequirements).toHaveBeenCalledTimes(2))
    expect(vi.mocked(confirmRequirements).mock.calls[0]).toEqual(
      vi.mocked(confirmRequirements).mock.calls[1],
    )
  })
  it.each([401, 403, 409, 503])(
    'withholds policy choice for source error %i without opting in',
    async (status) => {
      vi.mocked(getSdlcRoutingPolicy).mockRejectedValue(new SdlcRequestError(status))
      mount()
      await screen.findByRole('alert')
      if (status === 409)
        expect(
          screen.getByText(
            'The revision or retained SDLC binding is inconsistent; refresh required',
          ),
        ).toBeInTheDocument()
      expect(
        screen.queryByRole('checkbox', { name: /Publish with routing/ }),
      ).not.toBeInTheDocument()
      expect(confirmRequirements).not.toHaveBeenCalled()
    },
  )
  it('withholds choice while source loads and never fetches it for a non-owner', async () => {
    vi.mocked(getSdlcRoutingPolicy).mockReturnValue(new Promise(() => {}))
    const view = mount()
    await screen.findByRole('status', { name: 'Loading published routing policy' })
    expect(screen.queryByRole('checkbox', { name: /Publish with routing/ })).not.toBeInTheDocument()
    view.unmount()
    vi.mocked(getSdlcRoutingPolicy).mockClear()
    vi.mocked(getSdlcSnapshot).mockResolvedValue({
      ...initial,
      context: {
        ...initial.context,
        permissions: { can_answer: false, can_confirm: false },
      },
    })
    mount()
    await screen.findByText('Owner confirmation is unavailable')
    expect(getSdlcRoutingPolicy).not.toHaveBeenCalled()
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
  })
  it.each([403, 409, 503])(
    'blocks already selected publication on policy refresh error %i',
    async (status) => {
      vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(routingPolicy(task))
      mount()
      fireEvent.click(
        await screen.findByRole('checkbox', { name: 'Publish with routing policy version 5' }),
      )
      fireEvent.click(
        screen.getByRole('checkbox', { name: 'I confirm the requirements of revision 2' }),
      )
      vi.mocked(getSdlcRoutingPolicy).mockRejectedValue(new SdlcRequestError(status))
      fireEvent.click(screen.getByRole('button', { name: 'Refresh SDLC state' }))
      await screen.findByRole('alert')
      expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
      expect(confirmRequirements).not.toHaveBeenCalled()
      if (status === 403)
        expect(
          screen.queryByRole('checkbox', { name: /Publish with routing/ }),
        ).not.toBeInTheDocument()
      else
        expect(
          screen.getByRole('checkbox', { name: 'Publish with routing policy version 5' }),
        ).toBeChecked()
    },
  )
  it('does not transfer publication choice to a new requirement pin', async () => {
    vi.mocked(getSdlcRoutingPolicy).mockResolvedValue(routingPolicy(task))
    mount()
    fireEvent.click(
      await screen.findByRole('checkbox', { name: 'Publish with routing policy version 5' }),
    )
    vi.mocked(getSdlcSnapshot).mockResolvedValue({
      ...initial,
      context: { ...initial.context, requirement_revision: 3 },
      requirements: { ...initial.requirements!, revision: 3 },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Refresh SDLC state' }))
    await screen.findByRole('checkbox', { name: 'I confirm the requirements of revision 3' })
    await waitFor(() =>
      expect(
        screen.getByRole('checkbox', { name: 'Publish with routing policy version 5' }),
      ).not.toBeChecked(),
    )
    expect(screen.getByRole('button', { name: 'Confirm requirements' })).toBeDisabled()
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
  it('renders the accepted analyst result as its own translated history event', async () => {
    vi.mocked(getSdlcSnapshot).mockResolvedValue(queued)
    vi.mocked(getSdlcMetadata).mockResolvedValue({
      contract_version: 1,
      projection: 'metadata_v1',
      after: '0',
      next_after: '19',
      has_more: false,
      events: [
        {
          ...event,
          event_type: 'analysis.result_accepted',
          payload: {
            ...event.payload,
            stage: 'Architecture',
            resource: {
              architecture_input_sha256: hash,
              assignment_id: confirmationId,
              execution_id: intentId,
              result_id: task,
              terminal_receipt_ref: 'qa://analysis/receipt',
              terminal_receipt_sha256: hash,
              workflow_task_ref: 'SDLC-3',
            },
          },
        },
      ],
    })
    mount()
    const history = await screen.findByRole('region', { name: 'SDLC events' })
    expect(await within(history).findByText('Analysis result accepted')).toBeInTheDocument()
    expect(within(history).queryByText('Analysis intent queued')).not.toBeInTheDocument()
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
  it.each([
    ['delivery.queued', 'Delivery queued', 'Работа поставлена в очередь'],
    ['delivery.reconciled', 'Delivery reconciled', 'Состояние выполнения сверено'],
    [
      'delivery.assignment_reserved',
      'Delivery assignment prepared, awaiting admission',
      'Назначение подготовлено, допуск ожидается',
    ],
    ['delivery.lease_renewed', 'Delivery lease renewed', 'Аренда назначения продлена'],
    ['delivery.run_bound', 'Delivery run bound', 'Запуск привязан к назначению'],
    ['delivery.result_accepted', 'Delivery result accepted', 'Результат выполнения принят'],
  ] as const)(
    'translates lifecycle history %s without granting owner actions',
    async (kind, en, ru) => {
      const assigned = [
        'delivery.assignment_reserved',
        'delivery.lease_renewed',
        'delivery.run_bound',
      ].includes(kind)
      const accepted = kind === 'delivery.result_accepted'
      const lifecycleEvent: MetadataEvent = {
        ...event,
        event_type: kind,
        payload: {
          ...event.payload,
          resource: {
            task_version: 10,
            status: assigned ? 'InProgress' : 'Ready',
            runtime: kind === 'delivery.run_bound' ? 'running' : assigned ? 'reserved' : 'queued',
            assignment_id: assigned ? confirmationId : null,
            execution_id: assigned ? intentId : null,
            agent_id: assigned ? task : null,
            fencing_token: assigned ? 3 : null,
            lease_id: assigned ? task : null,
            lease_version: assigned ? 2 : null,
            assignment_hash: assigned ? hash : null,
            result_id: accepted ? task : null,
            previous_result_id: accepted ? task : null,
            result_sha256: accepted ? hash : null,
          },
        },
      }
      vi.mocked(getSdlcSnapshot).mockResolvedValue(queued)
      vi.mocked(getSdlcMetadata).mockResolvedValue({
        contract_version: 1,
        projection: 'metadata_v1',
        after: '0',
        next_after: '19',
        has_more: false,
        events: [lifecycleEvent],
      })
      mount()
      const history = await screen.findByRole('region', { name: 'SDLC events' })
      expect(await within(history).findByText(en)).toBeInTheDocument()
      await act(() => i18n.changeLanguage('ru'))
      expect(await within(history).findByText(ru)).toBeInTheDocument()
      expect(within(history).queryByText(/sdlc\.events\./)).not.toBeInTheDocument()
      expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
      expect(confirmRequirements).not.toHaveBeenCalled()
    },
  )
  it.each([
    {
      event: {
        ...event,
        event_type: 'delivery.proposal_submitted',
        payload: {
          ...event.payload,
          resource: {
            proposal_id: intentId,
            assignment_id: confirmationId,
            execution_id: intentId,
            agent_id: task,
            output_sha256: hash,
          },
        },
      },
      en: 'Delivery proposal submitted',
      ru: 'Предложение результата сохранено',
    },
    {
      event: {
        ...event,
        event_type: 'pm.feedback_recorded',
        payload: {
          ...event.payload,
          resource: {
            message_id: intentId,
            content_hash: hash,
            fence: {
              assignment_id: confirmationId,
              execution_id: intentId,
              agent_id: task,
              assignment_version: 2,
            },
          },
        },
      },
      en: 'PM feedback recorded',
      ru: 'Сообщение владельца для PM сохранено',
    },
    {
      event: {
        ...event,
        event_type: 'pm.completed',
        payload: {
          ...event.payload,
          resource: {
            completion_id: intentId,
            fence: {
              assignment_id: confirmationId,
              execution_id: intentId,
              agent_id: task,
              assignment_version: 2,
            },
            lease_id: task,
            lease_version: 2,
            requirement_revision: 2,
            content_hash: hash,
            terminal_receipt_sha256: hash,
            workflow_receipt_sha256: hash,
          },
        },
      },
      en: 'PM execution completed',
      ru: 'Выполнение PM завершено',
    },
  ] satisfies { event: MetadataEvent; en: string; ru: string }[])(
    'translates $event.event_type without treating it as owner confirmation',
    async ({ event: recorded, en, ru }) => {
      vi.mocked(getSdlcSnapshot).mockResolvedValue(queued)
      vi.mocked(getSdlcMetadata).mockResolvedValue({
        contract_version: 1,
        projection: 'metadata_v1',
        after: '0',
        next_after: '19',
        has_more: false,
        events: [recorded],
      })
      mount()
      const history = await screen.findByRole('region', { name: 'SDLC events' })
      expect(await within(history).findByText(en)).toBeInTheDocument()
      await act(() => i18n.changeLanguage('ru'))
      expect(await within(history).findByText(ru)).toBeInTheDocument()
      expect(within(history).queryByText(/sdlc\.events\./)).not.toBeInTheDocument()
      expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
      expect(confirmRequirements).not.toHaveBeenCalled()
    },
  )
})
