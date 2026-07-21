import { formatDuration } from '@/shared/lib/time'
import { Progress } from '@/shared/ui/progress'
import { Button } from '@/shared/ui/button'
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

  const estimate = originalEstimateSeconds ?? timeSpentSeconds
  const remaining = remainingEstimateSeconds ?? Math.max(0, estimate - timeSpentSeconds)
  const over = timeSpentSeconds > estimate
  const progressVariant: 'default' | 'danger' = over ? 'danger' : 'default'

  return (
    <div className="space-y-3">
      <Progress value={timeSpentSeconds} max={estimate || 1} variant={progressVariant} />
      <div
        className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-text-secondary"
        data-testid="time-tracking-summary"
      >
        <span className="font-medium text-text-primary">{formatDuration(timeSpentSeconds)}</span>
        <span className="text-text-muted">{' '}{t('timeTracking.spent')}</span>
        <span className="text-text-muted">{' / '}</span>
        <span className="font-medium text-text-primary">{estimate > 0 ? formatDuration(estimate) : '-'}</span>
        <span className="text-text-muted">{' '}{t('timeTracking.estimated')}</span>
        {remainingEstimateSeconds !== null && (
          <>
            <span className="text-text-muted">{' / '}</span>
            <span className="font-medium text-text-primary">{formatDuration(remaining)}</span>
            <span className="text-text-muted">{' '}{t('timeTracking.remaining')}</span>
          </>
        )}
        {over && (
          <span className="text-danger">
            {' '}{t('timeTracking.overBy')} {formatDuration(timeSpentSeconds - estimate)}
          </span>
        )}
      </div>
      <Button variant="secondary" size="sm" className="w-full" onClick={onLogWork}>
        {t('timeTracking.logWork')}
      </Button>
    </div>
  )
}
