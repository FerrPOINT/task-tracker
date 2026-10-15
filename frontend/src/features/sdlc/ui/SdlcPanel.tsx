import { useRef, useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { ArrowLeft, ArrowRight, Check, RefreshCw } from 'lucide-react'
import {
  Button,
  DescriptionItem,
  EmptyState,
  ErrorState,
  Label,
  LoadingState,
  ResourceList,
  ResourceListRow,
} from '@pdlc/ui/ui'
import {
  confirmRequirements,
  routingRoles,
  SdlcRequestError,
  type MetadataEvent,
  type RequirementsRevision,
  type ConfirmCommand,
  type RoutingPolicy,
} from '@/api/sdlc'
import { useSdlcTask } from '../model/use-sdlc'
import {
  canConfirmRequirementsContext,
  isRequirementsReviewContext,
} from '../model/confirmation-context'

const documentFields = [
  'goal',
  'scope',
  'exclusions',
  'scenarios',
  'acceptance_criteria',
  'constraints',
  'dependencies',
  'assumptions',
  'checklist',
  'prerequisites',
] as const
const eventLabels = {
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

export function SdlcPanel({ issueId }: { issueId: string }) {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const [cursors, setCursors] = useState(['0'])
  const after = cursors[cursors.length - 1] ?? '0'
  const { snapshot, metadata, routingPolicy, queryKey, authenticated } = useSdlcTask(issueId, after)
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
  const refresh = async () => {
    const result = await snapshot.refetch()
    if (result.data && !result.error) {
      await metadata.refetch()
      if (canConfirmRequirementsContext(result.data.context)) await routingPolicy.refetch()
    }
  }
  if (!authenticated) return <ErrorState message={t('sdlc.errors.401')} />
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
    <section aria-label={t('sdlc.title')} className="min-w-0 border-t border-border pt-4 text-sm">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-base font-semibold">{t('sdlc.title')}</h2>
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
      </div>
      <p role="status" className="mb-3 text-text-muted">
        {snapshot.isFetching
          ? t('sdlc.refreshing')
          : snapshot.error
            ? t('sdlc.stale')
            : t('sdlc.readAt')}{' '}
        <time dateTime={new Date(snapshot.dataUpdatedAt).toISOString()}>
          {new Date(snapshot.dataUpdatedAt).toLocaleString()}
        </time>
      </p>
      {snapshot.error && (
        <ErrorState message={errorMessage(snapshot.error)} onRetry={() => void refresh()} />
      )}
      <dl className="mb-4 min-w-0">
        <DescriptionItem layout="inline" label={t('sdlc.stage')}>
          {context.stage}
        </DescriptionItem>
        <DescriptionItem layout="inline" label={t('sdlc.reason')}>
          {context.waiting_reason
            ? t(`sdlc.reasons.${context.waiting_reason}`, { defaultValue: context.waiting_reason })
            : t('sdlc.noReason')}
        </DescriptionItem>
        <DescriptionItem layout="inline" label={t('sdlc.instance')}>
          {context.tracker_instance_id}
        </DescriptionItem>
        <DescriptionItem layout="inline" label={t('sdlc.project')}>
          {context.project_id}
        </DescriptionItem>
        <DescriptionItem layout="inline" label={t('sdlc.root')}>
          {context.root_task_id}
        </DescriptionItem>
        <DescriptionItem layout="inline" label={t('sdlc.owner')}>
          {context.owner_subject}
        </DescriptionItem>
      </dl>

      {context.stage === 'Analysis' &&
        (intent ? (
          <section aria-label={t('sdlc.intent')} className="mb-5 border-y border-border py-4">
            <h3 className="mb-2 font-semibold">{t('sdlc.intent')}</h3>
            <p className="mb-3 font-medium text-text-primary">
              {intent.stage} / {intent.status} · {t('sdlc.awaitingAdmission')}
            </p>
            <dl>
              <DescriptionItem layout="inline" label={t('sdlc.intentRef')}>
                {intent.intent_id}
              </DescriptionItem>
              <DescriptionItem layout="inline" label={t('sdlc.confirmationRef')}>
                {intent.confirmation_id}
              </DescriptionItem>
              <DescriptionItem layout="inline" label={t('sdlc.revision')}>
                {intent.requirement_revision}
              </DescriptionItem>
              <DescriptionItem layout="inline" label="SHA-256">
                {intent.content_hash}
              </DescriptionItem>
              <DescriptionItem layout="inline" label={t('sdlc.operationKey')}>
                {intent.operation_key}
              </DescriptionItem>
              <DescriptionItem layout="inline" label={t('sdlc.routing')}>
                {intent.role} / {intent.workflow} / {intent.mode} / {intent.scope}
              </DescriptionItem>
              <DescriptionItem layout="inline" label={t('sdlc.cycleAttempt')}>
                {intent.cycle} / {intent.attempt}
              </DescriptionItem>
              <DescriptionItem layout="inline" label={t('sdlc.createdAt')}>
                <time dateTime={intent.created_at}>
                  {new Date(intent.created_at).toLocaleString()}
                </time>
              </DescriptionItem>
            </dl>
          </section>
        ) : (
          <ErrorState message={t('sdlc.noIntent')} onRetry={() => void snapshot.refetch()} />
        ))}

      {requirements ? (
        <section aria-label={t('sdlc.requirements')} className="min-w-0">
          <h3 className="mb-2 font-semibold">
            {t('sdlc.requirements')} · {t('sdlc.revision')} {requirements.revision}
          </h3>
          <dl className="mb-4">
            <DescriptionItem layout="inline" label="SHA-256">
              {requirements.content_hash}
            </DescriptionItem>
          </dl>
          {documentFields.map((field) => (
            <div key={field} className="mb-4 min-w-0">
              <h4 className="mb-1 font-medium">{t(`sdlc.document.${field}`)}</h4>
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
            <div className="border-t border-border py-4">
              {confirm.error && <ErrorState message={errorMessage(confirm.error)} />}
              {confirm.data && <p role="status">{t('sdlc.confirmationSaved')}</p>}
              {context.permissions.can_confirm ? (
                <>
                  <Label className="mb-3 flex min-h-10 items-start gap-2">
                    <input
                      type="checkbox"
                      className="mt-1 h-4 w-4 shrink-0 accent-accent"
                      checked={acknowledgedPin === pin}
                      disabled={!canConfirm}
                      onChange={(event) => setAcknowledgedPin(event.target.checked ? pin : null)}
                    />
                    <span>{t('sdlc.acknowledge', { revision: requirements.revision })}</span>
                  </Label>
                  <section aria-label={t('sdlc.publication.title')} className="mb-4 min-w-0">
                    <h4 className="mb-2 font-medium">{t('sdlc.publication.title')}</h4>
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
                            <details className="mb-3 min-w-0">
                              <summary>{t('sdlc.publication.references')}</summary>
                              <dl>
                                <DescriptionItem layout="inline" label="SHA-256">
                                  {displayedPolicy.routing_hash}
                                </DescriptionItem>
                              </dl>
                              {Object.values(routingRoles).map((role) => {
                                const route = displayedPolicy.routes[role]
                                return (
                                  <section key={role} className="border-b border-border py-2">
                                    <h5 className="mb-1 font-medium">
                                      {t(`sdlc.publication.roles.${role}`)}
                                    </h5>
                                    <dl>
                                      <DescriptionItem
                                        layout="inline"
                                        label={t('sdlc.publication.agent')}
                                      >
                                        {route.agent_id}
                                      </DescriptionItem>
                                      <DescriptionItem
                                        layout="inline"
                                        label={t('sdlc.publication.effectiveRevision')}
                                      >
                                        {route.fleet_config_revision}
                                      </DescriptionItem>
                                      {displayedPolicy.contract_version === 1 ? (
                                        <DescriptionItem layout="inline" label={t('sdlc.publication.package')}>
                                          {route.package_commit} / {route.package_manifest_sha256}
                                        </DescriptionItem>
                                      ) : (
                                        <DescriptionItem layout="inline" label={t('sdlc.publication.instructionProfile')}>
                                          {route.instruction_profile} /{' '}
                                          {route.instruction_manifest_sha256}
                                        </DescriptionItem>
                                      )}
                                      <DescriptionItem
                                        layout="inline"
                                        label={t('sdlc.publication.workflow')}
                                      >
                                        {route.namespace_id} / {route.namespace_name} /{' '}
                                        {route.workflow_id} / {route.workflow_key}
                                      </DescriptionItem>
                                      <DescriptionItem
                                        layout="inline"
                                        label={t('sdlc.publication.profile')}
                                      >
                                        {route.profile} / {route.workflow_catalog_version} /{' '}
                                        {route.workflow_catalog_sha256}
                                      </DescriptionItem>
                                    </dl>
                                  </section>
                                )
                              })}
                            </details>
                            <Label className="flex min-h-10 items-start gap-2">
                              <input
                                type="checkbox"
                                className="mt-1 h-4 w-4 shrink-0 accent-accent"
                                checked={Boolean(selected)}
                                disabled={!canConfirm || (!selected && !policyReady)}
                                onChange={(event) => {
                                  setPublication(
                                    event.target.checked && pin && policyReady
                                      ? { pin, policy: routingPolicy.data! }
                                      : null,
                                  )
                                  setConflictedPublication(null)
                                }}
                              />
                              <span>
                                {t('sdlc.publication.select', { version: displayedPolicy.version })}
                              </span>
                            </Label>
                            {selected && !publicationReady && (
                              <p role="status" className="mt-2 text-text-muted">
                                {t('sdlc.publication.stale')}
                              </p>
                            )}
                          </>
                        )}
                  </section>
                  <Button
                    disabled={!canConfirm || acknowledgedPin !== pin || !publicationReady}
                    onClick={() => submitConfirmation(requirements)}
                    className="min-h-10 whitespace-normal"
                  >
                    <Check className="h-4 w-4 shrink-0" />
                    {confirm.isPending ? t('sdlc.confirming') : t('sdlc.confirm')}
                  </Button>
                </>
              ) : (
                <p className="text-text-muted">{t('sdlc.confirmUnavailable')}</p>
              )}
            </div>
          )}
        </section>
      ) : (
        <EmptyState message={t('sdlc.noRequirements')} />
      )}

      <section className="mt-4 min-w-0 border-t border-border pt-4" aria-label={t('sdlc.history')}>
        <h3 className="mb-2 font-semibold">{t('sdlc.history')}</h3>
        {metadata.dataUpdatedAt > 0 && (
          <p role="status" className="mb-2 text-text-muted">
            {metadata.isFetching
              ? t('sdlc.refreshing')
              : metadata.error
                ? t('sdlc.stale')
                : t('sdlc.readAt')}{' '}
            <time dateTime={new Date(metadata.dataUpdatedAt).toISOString()}>
              {new Date(metadata.dataUpdatedAt).toLocaleString()}
            </time>
          </p>
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
            <p className="mb-2 text-text-muted">
              {t('sdlc.afterCursor', { cursor: metadata.data.after })}
            </p>
            {metadata.data.events.length === 0 ? (
              <EmptyState message={t('sdlc.noEvents')} />
            ) : (
              <ResourceList asChild>
                <ol>
                  {metadata.data.events.map((event) => (
                    <ResourceListRow asChild key={event.event_id}>
                      <li className="min-w-0">
                        <p className="font-medium">
                          {t(`sdlc.events.${eventLabels[event.event_type]}`)}
                        </p>
                        <p className="text-text-muted">
                          #{event.sequence} · {new Date(event.created_at).toLocaleString()}
                        </p>
                        {event.event_type === 'analysis.intent_created' && (
                          <dl>
                            <DescriptionItem layout="inline" label={t('sdlc.intentRef')}>
                              {event.payload.resource.intent_id}
                            </DescriptionItem>
                            <DescriptionItem layout="inline" label={t('sdlc.revision')}>
                              {event.payload.resource.requirement_revision}
                            </DescriptionItem>
                          </dl>
                        )}
                        <details className="min-w-0 text-text-muted">
                          <summary>{t('sdlc.eventPin')}</summary>
                          <dl>
                            <DescriptionItem layout="inline" label={t('sdlc.eventRef')}>
                              {event.event_id}
                            </DescriptionItem>
                            <DescriptionItem layout="inline" label="SHA-256">
                              {event.metadata_sha256}
                            </DescriptionItem>
                          </dl>
                        </details>
                      </li>
                    </ResourceListRow>
                  ))}
                </ol>
              </ResourceList>
            )}
            <div className="mt-3 flex gap-2">
              <Button
                variant="secondary"
                size="icon"
                aria-label={t('sdlc.previous')}
                title={t('sdlc.previous')}
                disabled={cursors.length === 1 || metadata.isFetching}
                onClick={() => setCursors((current) => current.slice(0, -1))}
              >
                <ArrowLeft className="h-4 w-4" />
              </Button>
              <Button
                variant="secondary"
                size="icon"
                aria-label={t('sdlc.next')}
                title={t('sdlc.next')}
                disabled={!metadata.data.has_more || metadata.isFetching}
                onClick={() => setCursors((current) => [...current, metadata.data!.next_after])}
              >
                <ArrowRight className="h-4 w-4" />
              </Button>
            </div>
          </>
        ) : null}
      </section>
    </section>
  )
}
