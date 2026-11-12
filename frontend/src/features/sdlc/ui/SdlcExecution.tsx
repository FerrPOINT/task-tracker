import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import {
  CopyButton,
  DescriptionItem,
  Disclosure,
  ErrorState,
  JsonBlock,
  LoadingState,
} from '@pdlc/ui/ui'
import { getSdlcLifecycle, SdlcRequestError, type SdlcContext } from '@/api/sdlc'

export function SdlcExecution({
  context,
  queryKey,
  enabled,
}: {
  context: SdlcContext
  queryKey: readonly unknown[]
  enabled: boolean
}) {
  const { t } = useTranslation()
  // Draft/Backlog belong to PM preparation; delivery storage is created when development starts.
  const preparing = ['Draft', 'Clarification', 'Backlog'].includes(context.stage)
  const lifecycle = useQuery({
    queryKey: [...queryKey, 'lifecycle', context.stage, context.status, context.previous_result_id],
    queryFn: ({ signal }) => getSdlcLifecycle(context, signal),
    enabled: enabled && !preparing && context.status != null,
    retry: false,
    gcTime: 0,
  })
  if (!enabled) return <p>{t('sdlc.execution.refreshRequired')}</p>
  if (preparing)
    return (
      <div>
        <p>
          {t(context.stage === 'Backlog' ? 'sdlc.execution.backlog' : 'sdlc.execution.preparing')}
        </p>
        {context.stage !== 'Backlog' && context.assignment && (
          <DescriptionItem layout="inline" label={t('sdlc.execution.assignee')}>
            {t('sdlc.publication.roles.project_manager')}
          </DescriptionItem>
        )}
      </div>
    )
  if (context.status == null) return <p>{t('sdlc.execution.notPublished')}</p>
  if (lifecycle.error)
    return (
      <ErrorState
        message={t(
          `sdlc.errors.${lifecycle.error instanceof SdlcRequestError ? lifecycle.error.status : 'unknown'}`,
          { defaultValue: t('sdlc.errors.unknown') },
        )}
        onRetry={() => void lifecycle.refetch()}
      />
    )
  if (lifecycle.isPending) return <LoadingState message={t('sdlc.execution.loading')} />
  const current = lifecycle.data
  const assignment = current.receipt.assignment
  return (
    <div className="min-w-0">
      <dl>
        <DescriptionItem layout="inline" label={t('sdlc.execution.title')}>
          {t(`sdlc.runtime.${current.runtime_state}`)}
        </DescriptionItem>
        <DescriptionItem layout="inline" label={t('sdlc.execution.assignee')}>
          {assignment && assignment.stage === context.stage
            ? t(`sdlc.publication.roles.${assignment.role}`)
            : t('sdlc.execution.unassigned')}
        </DescriptionItem>
      </dl>
      <Disclosure summary={t('sdlc.technicalDetails')}>
        <JsonBlock value={current} />
        <CopyButton value={JSON.stringify(current, null, 2)}>{t('sdlc.execution.copy')}</CopyButton>
      </Disclosure>
    </div>
  )
}
