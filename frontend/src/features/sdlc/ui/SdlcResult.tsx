import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { Button, CopyButton, Disclosure, ErrorState, LoadingState, JsonBlock } from '@pdlc/ui/ui'
import { getSdlcResult, SdlcRequestError } from '@/api/sdlc'

export function SdlcResult({
  issueId,
  resultId,
  queryKey,
  enabled,
}: {
  issueId: string
  resultId: string
  queryKey: readonly unknown[]
  enabled: boolean
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const result = useQuery({
    queryKey: [...queryKey, 'result', resultId],
    queryFn: ({ signal }) => getSdlcResult(issueId, resultId, signal),
    enabled: open && enabled,
    retry: false,
    gcTime: 0,
  })
  const value = enabled && !result.error ? result.data : undefined
  return (
    <div className="min-w-0">
      <Button
        variant="secondary"
        size="sm"
        disabled={!enabled}
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        {t(open ? 'sdlc.results.close' : 'sdlc.results.open')}
      </Button>
      {open && enabled && (
        <section aria-label={t('sdlc.results.detail')} className="mt-3 min-w-0">
          {result.error ? (
            <ErrorState
              message={t(
                `sdlc.errors.${result.error instanceof SdlcRequestError ? result.error.status : 'unknown'}`,
                { defaultValue: t('sdlc.errors.unknown') },
              )}
              onRetry={() => void result.refetch()}
            />
          ) : result.isPending ? (
            <LoadingState message={t('sdlc.results.loading')} />
          ) : (
            value && (
              <>
                <p>
                  {t(`board.sdlcStage.${value.receipt.previous_stage}`)} ·{' '}
                  <time dateTime={value.receipt.created_at}>
                    {new Date(value.receipt.created_at).toLocaleString()}
                  </time>
                </p>
                {value.source === 'legacy_analysis' ? (
                  <p className="whitespace-pre-wrap [overflow-wrap:anywhere]">
                    {value.receipt.summary}
                  </p>
                ) : (
                  <>
                    {'verdict' in value.receipt.output && (
                      <p>{t(`sdlc.results.verdict.${value.receipt.output.verdict}`)}</p>
                    )}
                    <p>{t('sdlc.results.accepted')}</p>
                  </>
                )}
                <p>{t('sdlc.results.materials')}</p>
                <Disclosure summary={t('sdlc.technicalDetails')}>
                  <JsonBlock value={value} />
                  <CopyButton value={JSON.stringify(value, null, 2)}>
                    {t('sdlc.results.copy')}
                  </CopyButton>
                </Disclosure>
              </>
            )
          )}
        </section>
      )}
    </div>
  )
}
