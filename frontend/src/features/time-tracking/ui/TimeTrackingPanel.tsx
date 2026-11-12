import { formatDuration } from '@/shared/lib/time'
import { Progress, Button } from '@pdlc/ui/ui'

import { useTranslation } from 'react-i18next'

interface TimeTrackingPanelProps {
  timeSpentSeconds: number
  originalEstimateSeconds: number | null
  remainingEstimateSeconds: number | null
  onLogWork: () => void
}

export function TimeTrackingPanel({
  timeSpentSeconds,
  originalEstimateSeconds,
  remainingEstimateSeconds,
  onLogWork,
}: TimeTrackingPanelProps) {
  const { t } = useTranslation()

  const estimate = originalEstimateSeconds
  const hasEstimate = estimate !== null && estimate > 0
  const overBy = hasEstimate ? Math.max(0, timeSpentSeconds - estimate) : 0
  const over = overBy > 0
  const progressVariant: 'default' | 'danger' = over ? 'danger' : 'default'

  return (
    <div className="space-y-3">
      {hasEstimate && (
        <Progress
          value={timeSpentSeconds}
          max={estimate}
          variant={progressVariant}
          aria-label={`${t('timeTracking.spent')} / ${t('timeTracking.estimated')}`}
          aria-valuetext={`${formatDuration(timeSpentSeconds)} / ${formatDuration(estimate)}`}
        />
      )}
      <div
        className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-text-secondary"
        data-testid="time-tracking-summary"
      >
        <span className="font-medium text-text-primary">{formatDuration(timeSpentSeconds)}</span>
        <span className="text-text-muted"> {t('timeTracking.spent')}</span>
        {hasEstimate ? (
          <>
            <span className="text-text-muted">{' / '}</span>
            <span className="font-medium text-text-primary">{formatDuration(estimate)}</span>
            <span className="text-text-muted"> {t('timeTracking.estimated')}</span>
          </>
        ) : (
          <span className="text-text-muted">{t('timeTracking.noEstimate')}</span>
        )}
        {remainingEstimateSeconds !== null && (
          <>
            <span className="text-text-muted">{' / '}</span>
            <span className="font-medium text-text-primary">
              {formatDuration(remainingEstimateSeconds)}
            </span>
            <span className="text-text-muted"> {t('timeTracking.remaining')}</span>
          </>
        )}
        {over && (
          <span className="text-danger">
            {' '}
            {t('timeTracking.overBy')} {formatDuration(overBy)}
          </span>
        )}
      </div>
      <Button variant="secondary" size="sm" className="w-full" onClick={onLogWork}>
        {t('timeTracking.logWork')}
      </Button>
    </div>
  )
}
