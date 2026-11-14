import { ContentSection } from '@pdlc/ui/ui'
import { useScopedMutation as useMutation } from '@/shared/api/useScopedMutation'
import {
  ResourceListDescription,
  ResourceListTitle,
  PageNavigation,
  Fieldset,
  Notice,
  SectionHeader,
} from '@pdlc/ui/ui'
import {
  Button,
  DescriptionItem,
  EmptyState,
  ErrorState,
  LoadingState,
  ResourceList,
  ResourceListRow,
  Checkbox,
} from '@pdlc/ui/ui'
import { useRef, useState, type ReactNode } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { Check, RefreshCw } from 'lucide-react'

import {
  confirmRequirements,
  routingRoles,
  SdlcRequestError,
  type MetadataEvent,
  type RequirementsRevision,
  type ConfirmCommand,
  type RoutingPolicy,
} from '@/api/sdlc'
import { SdlcQuestions } from './SdlcQuestions'
import { SdlcExecution } from './SdlcExecution'
import { SdlcResult } from './SdlcResult'
import { useSdlcTask } from '../model/use-sdlc'
import type { IssueDetail } from '@/api/issue'
import {
  canConfirmRequirementsContext,
  isRequirementsReviewContext,
} from '../model/confirmation-context'

const documentFields = [
  'goal',
  'scope',
  'acceptance_criteria',
  'exclusions',
  'scenarios',
  'constraints',
  'dependencies',
  'assumptions',
  'checklist',
  'prerequisites',
] as const
const eventLabels = {
  'source.selected': 'sourceSelected',
  'delivery.queued': 'deliveryQueued',
  'delivery.reconciled': 'deliveryReconciled',
  'delivery.assignment_reserved': 'deliveryAssignmentPrepared',
  'delivery.lease_renewed': 'deliveryLeaseRenewed',
  'delivery.run_bound': 'deliveryRunBound',
  'delivery.result_accepted': 'deliveryResultAccepted',
  'delivery.proposal_submitted': 'deliveryProposalSubmitted',
  'pm.feedback_recorded': 'pmFeedbackRecorded',
  'pm.completed': 'pmCompleted',
  'task.created': 'taskCreated',
  'task.bound': 'taskBound',
  'pm.assigned': 'pmAssigned',
  'clarification.published': 'questionPublished',
  'clarification.cancelled': 'questionCancelled',
  'clarification.answered': 'questionAnswered',
  'requirements.published': 'requirementsPublished',
  'requirements.evidence_recorded': 'evidenceRecorded',
  'requirements.confirmed': 'requirementsConfirmed',
  'analysis.intent_created': 'intentQueued',
  'analysis.assignment_reserved': 'assignmentPrepared',
  'analysis.result_accepted': 'analysisResultAccepted',
} satisfies Record<MetadataEvent['event_type'], string>

export function SdlcPanel({
  issueId,
  lifecycle,
  materials,
}: {
  issueId: string
  lifecycle: IssueDetail['lifecycle']
  materials?: ReactNode
}) {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const [cursors, setCursors] = useState(['0'])
  const after = cursors[cursors.length - 1] ?? '0'
  const { snapshot, metadata, routingPolicy, queryKey, authenticated } = useSdlcTask(
    issueId,
    after,
    lifecycle,
  )
  const [acknowledgedPin, setAcknowledgedPin] = useState<string | null>(null)
  const [publication, setPublication] = useState<{ pin: string; policy: RoutingPolicy } | null>(
    null,
  )
  const [conflictedPublication, setConflictedPublication] = useState<string | null>(null)
  const operationKeys = useRef(new Map<string, string>())
  const confirm = useMutation({
    mutationFn: ({ revision, command }: { revision: number; command: ConfirmCommand }) =>
      confirmRequirements(issueId, revision, command),
    retry: false,
    onError: (error, variables) => {
      if (
        error instanceof SdlcRequestError &&
        error.status === 409 &&
        variables.command.expected_routing_policy_version != null
      ) {
        setConflictedPublication(
          `${variables.command.expected_routing_policy_version}:${variables.command.content_hash}`,
        )
      }
    },
    onSettled: async () => {
      await snapshot.refetch()
      await refreshDescription()
      await qc.invalidateQueries({ queryKey: [...queryKey, 'metadata'] })
      for (const key of [['issue', issueId], ['project'], ['backlog']]) {
        await qc.invalidateQueries({ queryKey: key })
      }
    },
  })

  const errorMessage = (error: unknown) =>
    t(`sdlc.errors.${error instanceof SdlcRequestError ? error.status : 'unknown'}`, {
      defaultValue: t('sdlc.errors.unknown'),
    })
  const accessDenied =
    snapshot.error instanceof SdlcRequestError &&
    (snapshot.error.status === 401 || snapshot.error.status === 403)
  const refreshDescription = () =>
    qc.invalidateQueries({
      queryKey: ['sdlc', issueId],
      predicate: (query) => query.queryKey[3] === 'snapshot' && query.queryKey[2] !== queryKey[2],
    })
  const refresh = async () => {
    const result = await snapshot.refetch()
    await refreshDescription()
    if (result.data && !result.error) {
      await metadata.refetch()
      await qc.invalidateQueries({ queryKey: [...queryKey, 'questions'] })
      await qc.invalidateQueries({ queryKey: [...queryKey, 'lifecycle'] })
      await qc.invalidateQueries({ queryKey: [...queryKey, 'result'] })
      if (canConfirmRequirementsContext(result.data.context)) await routingPolicy.refetch()
    }
  }
  if (!authenticated) return <ErrorState message={t('sdlc.errors.401')} />
  if (lifecycle.enrolled === false) return <EmptyState message={t('sdlc.notEnrolled')} />
  if (snapshot.isPending) return <LoadingState message={t('sdlc.loading')} />
  if (accessDenied || (snapshot.error && !snapshot.data)) {
    return (
      <ErrorState message={errorMessage(snapshot.error)} onRetry={() => void snapshot.refetch()} />
    )
  }
  if (!snapshot.data)
    return (
      <EmptyState
        message={t('sdlc.noContext')}
        action={
          <Button
            variant="secondary"
            size="icon"
            title={t('sdlc.refresh')}
            aria-label={t('sdlc.refresh')}
            onClick={() => void snapshot.refetch()}
          >
            <RefreshCw className="h-4 w-4" />
          </Button>
        }
      />
    )

  const { context, requirements, intent } = snapshot.data
  const pin = requirements
    ? `${queryKey[2]}:${requirements.revision}:${requirements.content_hash}`
    : null
  const canConfirm =
    canConfirmRequirementsContext(context) &&
    requirements &&
    !snapshot.isFetching &&
    !snapshot.error &&
    !confirm.isPending
  const selected = publication?.pin === pin ? publication.policy : null
  const policyReady = Boolean(
    routingPolicy.data && !routingPolicy.isFetching && !routingPolicy.error,
  )
  const publicationReady =
    !selected ||
    (policyReady &&
      selected.version === routingPolicy.data?.version &&
      selected.routing_hash === routingPolicy.data?.routing_hash &&
      conflictedPublication !== `${selected.version}:${requirements?.content_hash}`)
  const displayedPolicy = selected ?? (policyReady ? routingPolicy.data : null)
  const submitConfirmation = (document: RequirementsRevision) => {
    if (!canConfirm || acknowledgedPin !== pin || !publicationReady) return
    const operationPin = `${pin}:${selected ? `${selected.version}:${selected.routing_hash}` : 'legacy'}`
    const key = operationKeys.current.get(operationPin) ?? crypto.randomUUID()
    operationKeys.current.set(operationPin, key)
    const command: ConfirmCommand = {
      content_hash: document.content_hash,
      idempotency_key: key,
      ...(selected ? { expected_routing_policy_version: selected.version } : {}),
    }
    confirm.mutate({ revision: document.revision, command })
  }

  return (
    <ContentSection asChild dividers="none">
      <section aria-label={t('sdlc.title')} className="min-w-0 text-sm">
        <SectionHeader
          title={t('sdlc.overview.title')}
          className="mb-3"
          actions={
            <Button
              variant="ghost"
              size="icon"
              title={t('sdlc.refresh')}
              aria-label={t('sdlc.refresh')}
              disabled={snapshot.isFetching || metadata.isFetching || confirm.isPending}
              onClick={() => void refresh()}
            >
              <RefreshCw className="h-4 w-4" />
            </Button>
          }
        />
        <Notice role="status" tone="neutral" appearance="inline">
          {snapshot.isFetching
            ? t('sdlc.refreshing')
            : snapshot.error
              ? t('sdlc.stale')
              : t('sdlc.readAt')}{' '}
          <time dateTime={new Date(snapshot.dataUpdatedAt).toISOString()}>
            {new Date(snapshot.dataUpdatedAt).toLocaleString()}
          </time>
        </Notice>
        {snapshot.error && (
          <ErrorState message={errorMessage(snapshot.error)} onRetry={() => void refresh()} />
        )}
        <section aria-label={t('sdlc.overview.now')} className="mb-4">
          <SectionHeader level={3} title={t('sdlc.overview.now')} className="mb-2" />
          <dl>
            <DescriptionItem layout="inline" label={t('sdlc.stage')}>
              {t(`board.sdlcStage.${context.stage}`)}
            </DescriptionItem>
            {context.status && (
              <DescriptionItem layout="inline" label={t('sdlc.overview.state')}>
                {t(`sdlc.status.${context.status}`, {
                  defaultValue: t('sdlc.overview.unknownState'),
                })}
              </DescriptionItem>
            )}
            <DescriptionItem layout="inline" label={t('sdlc.reason')}>
              {context.waiting_reason
                ? t(`sdlc.reasons.${context.waiting_reason}`, {
                    defaultValue: t('sdlc.unknownReason'),
                  })
                : t('sdlc.noReason')}
            </DescriptionItem>
          </dl>
          <SdlcExecution
            key={queryKey[2]}
            context={context}
            queryKey={queryKey}
            enabled={!snapshot.error && !snapshot.isFetching}
          />
        </section>
        <section aria-label={t('sdlc.nextStep.title')} className="mb-4">
          <SectionHeader level={3} title={t('sdlc.nextStep.title')} className="mb-2" />
          <p>
            {t(
              context.permissions.can_answer
                ? 'sdlc.nextStep.answer'
                : isRequirementsReviewContext(context)
                  ? context.permissions.can_confirm
                    ? 'sdlc.nextStep.confirm'
                    : 'sdlc.nextStep.owner'
                  : 'sdlc.nextStep.none',
            )}
          </p>
          {context.permissions.can_answer && (
            <SdlcQuestions
              key={queryKey[2]}
              context={context}
              queryKey={queryKey}
              enabled={!snapshot.error && !snapshot.isFetching}
              onChanged={refresh}
            />
          )}
        </section>

        {context.stage === 'Analysis' &&
          context.status == null &&
          (intent ? (
            <ContentSection asChild dividers="both">
              <section aria-label={t('sdlc.intent')} className="mb-5">
                <SectionHeader level={3} title={t('sdlc.intent')} className="mb-2" />
                <p className="mb-3 font-medium text-text-primary">{t('sdlc.awaitingAdmission')}</p>
                <dl>
                  <DescriptionItem layout="inline" label={t('sdlc.revision')}>
                    {intent.requirement_revision}
                  </DescriptionItem>
                  <DescriptionItem layout="inline" label={t('sdlc.createdAt')}>
                    <time dateTime={intent.created_at}>
                      {new Date(intent.created_at).toLocaleString()}
                    </time>
                  </DescriptionItem>
                </dl>
              </section>
            </ContentSection>
          ) : (
            <ErrorState message={t('sdlc.noIntent')} onRetry={() => void snapshot.refetch()} />
          ))}

        {requirements ? (
          <section aria-label={t('sdlc.requirements')} className="min-w-0">
            <SectionHeader
              level={3}
              title={
                <>
                  {t('sdlc.requirements')} · {t('sdlc.revision')} {requirements.revision}
                </>
              }
              className="mb-2"
            />
            {documentFields
              .filter((field) => {
                const value = requirements[field]
                return typeof value === 'string' ? value.trim().length > 0 : value.length > 0
              })
              .map((field) => (
                <div key={field} className="mb-4 min-w-0">
                  <SectionHeader
                    level={4}
                    title={<>{t(`sdlc.document.${field}`)}</>}
                    className="mb-1"
                  />
                  {typeof requirements[field] === 'string' ? (
                    <p className="whitespace-pre-wrap [overflow-wrap:anywhere]">
                      {requirements[field]}
                    </p>
                  ) : (
                    <ul className="list-inside list-disc space-y-1 [overflow-wrap:anywhere]">
                      {(requirements[field] as string[]).map((item, index) => (
                        <li key={index}>{item}</li>
                      ))}
                    </ul>
                  )}
                </div>
              ))}
            {isRequirementsReviewContext(context) && (
              <Fieldset separated legend={t('sdlc.confirm')} legendVisuallyHidden>
                {confirm.error && <ErrorState message={errorMessage(confirm.error)} />}
                {confirm.data && (
                  <Notice role="status" tone="success" appearance="inline">
                    {t('sdlc.confirmationSaved')}
                  </Notice>
                )}
                {context.permissions.can_confirm ? (
                  <>
                    <Checkbox
                      className="mb-3"
                      label={t('sdlc.acknowledge', { revision: requirements.revision })}
                      checked={acknowledgedPin === pin}
                      disabled={!canConfirm}
                      onCheckedChange={(checked) =>
                        setAcknowledgedPin(checked === true ? pin : null)
                      }
                    />
                    <section aria-label={t('sdlc.publication.title')} className="mb-4 min-w-0">
                      <SectionHeader
                        level={4}
                        title={<>{t('sdlc.publication.title')}</>}
                        className="mb-2"
                      />
                      {routingPolicy.isFetching && (
                        <LoadingState message={t('sdlc.publication.loading')} />
                      )}
                      {routingPolicy.error && (
                        <ErrorState
                          message={errorMessage(routingPolicy.error)}
                          onRetry={() => void routingPolicy.refetch()}
                        />
                      )}
                      {!routingPolicy.isPending && !routingPolicy.error && !routingPolicy.data && (
                        <p className="text-text-muted">{t('sdlc.publication.absent')}</p>
                      )}
                      {routingPolicy.error instanceof SdlcRequestError &&
                      [401, 403].includes(routingPolicy.error.status)
                        ? null
                        : displayedPolicy && (
                            <>
                              <p className="mb-2 text-text-muted">
                                {t('sdlc.publication.preparation')}
                              </p>
                              <dl className="mb-2">
                                <DescriptionItem layout="inline" label={t('sdlc.revision')}>
                                  {displayedPolicy.version}
                                </DescriptionItem>
                              </dl>
                              <ul
                                className="mb-3 list-inside list-disc"
                                aria-label={t('sdlc.publication.rolesTitle')}
                              >
                                {Object.values(routingRoles).map((role) => (
                                  <li key={role}>{t(`sdlc.publication.roles.${role}`)}</li>
                                ))}
                              </ul>
                              <Checkbox
                                label={t('sdlc.publication.select', {
                                  version: displayedPolicy.version,
                                })}
                                checked={Boolean(selected)}
                                disabled={!canConfirm || (!selected && !policyReady)}
                                onCheckedChange={(checked) => {
                                  setPublication(
                                    checked === true && pin && policyReady
                                      ? { pin, policy: routingPolicy.data! }
                                      : null,
                                  )
                                  setConflictedPublication(null)
                                }}
                              />
                              {selected && !publicationReady && (
                                <Notice role="status" tone="neutral" appearance="inline">
                                  {t('sdlc.publication.stale')}
                                </Notice>
                              )}
                            </>
                          )}
                    </section>
                    <Button
                      disabled={!canConfirm || acknowledgedPin !== pin || !publicationReady}
                      onClick={() => submitConfirmation(requirements)}
                      className="whitespace-normal"
                    >
                      <Check className="h-4 w-4 shrink-0" />
                      {confirm.isPending ? t('sdlc.confirming') : t('sdlc.confirm')}
                    </Button>
                  </>
                ) : (
                  <p className="text-text-muted">{t('sdlc.confirmUnavailable')}</p>
                )}
              </Fieldset>
            )}
          </section>
        ) : (
          <EmptyState message={t('sdlc.noRequirements')} />
        )}

        <section aria-label={t('sdlc.results.title')} className="my-4 min-w-0">
          <SectionHeader level={3} title={t('sdlc.results.title')} className="mb-2" />
          {context.previous_result_id ? (
            <SdlcResult
              key={queryKey[2] + context.previous_result_id}
              issueId={issueId}
              resultId={context.previous_result_id}
              queryKey={queryKey}
              enabled={!snapshot.error && !snapshot.isFetching}
            />
          ) : (
            <p className="text-text-muted">{t('sdlc.results.empty')}</p>
          )}
        </section>
        {materials && (
          <section aria-label={t('sdlc.results.materialsTitle')} className="my-4">
            <SectionHeader level={3} title={t('sdlc.results.materialsTitle')} className="mb-2" />
            {materials}
          </section>
        )}
        <ContentSection asChild dividers="start">
          <section className="mt-4 min-w-0" aria-label={t('sdlc.history')}>
            <SectionHeader level={3} title={<>{t('sdlc.history')}</>} className="mb-2" />
            {metadata.dataUpdatedAt > 0 && (
              <Notice role="status" tone="neutral" appearance="inline">
                {metadata.isFetching
                  ? t('sdlc.refreshing')
                  : metadata.error
                    ? t('sdlc.stale')
                    : t('sdlc.readAt')}{' '}
                <time dateTime={new Date(metadata.dataUpdatedAt).toISOString()}>
                  {new Date(metadata.dataUpdatedAt).toLocaleString()}
                </time>
              </Notice>
            )}
            {metadata.error ? (
              <ErrorState
                message={errorMessage(metadata.error)}
                onRetry={() => void metadata.refetch()}
              />
            ) : null}
            {metadata.isPending ? (
              <LoadingState message={t('sdlc.loading')} />
            ) : metadata.data && !metadata.error ? (
              <>
                {metadata.data.events.length === 0 ? (
                  <EmptyState message={t('sdlc.noEvents')} />
                ) : (
                  <ResourceList asChild>
                    <ol>
                      {metadata.data.events.map((event) => (
                        <ResourceListRow asChild key={event.event_id}>
                          <li className="min-w-0">
                            <ResourceListTitle asChild>
                              <p>{t(`sdlc.events.${eventLabels[event.event_type]}`)}</p>
                            </ResourceListTitle>
                            <ResourceListDescription asChild>
                              <p>
                                <time dateTime={event.created_at}>
                                  {new Date(event.created_at).toLocaleString()}
                                </time>
                              </p>
                            </ResourceListDescription>
                            {event.event_type === 'analysis.intent_created' && (
                              <dl>
                                <DescriptionItem layout="inline" label={t('sdlc.revision')}>
                                  {event.payload.resource.requirement_revision}
                                </DescriptionItem>
                              </dl>
                            )}
                            {event.event_type === 'delivery.result_accepted' &&
                              event.payload.resource.result_id && (
                                <SdlcResult
                                  key={queryKey[2]}
                                  issueId={issueId}
                                  resultId={event.payload.resource.result_id}
                                  queryKey={queryKey}
                                  enabled={
                                    !snapshot.error && !metadata.error && !metadata.isFetching
                                  }
                                />
                              )}
                          </li>
                        </ResourceListRow>
                      ))}
                    </ol>
                  </ResourceList>
                )}
                <PageNavigation
                  page={cursors.length}
                  hasNext={metadata.data.has_more}
                  onPageChange={(nextPage) =>
                    setCursors((current) =>
                      nextPage < current.length
                        ? current.slice(0, -1)
                        : [...current, metadata.data!.next_after],
                    )
                  }
                  label={t('sdlc.metadataPages', { defaultValue: 'Страницы метаданных SDLC' })}
                  previousLabel={t('sdlc.previous')}
                  nextLabel={t('sdlc.next')}
                  pending={metadata.isFetching}
                  compact
                  className="mt-3"
                />
              </>
            ) : null}
          </section>
        </ContentSection>
      </section>
    </ContentSection>
  )
}
