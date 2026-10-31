import { Badge } from '@pdlc/ui/ui'
import { useTranslation } from 'react-i18next'
import type { TaskLifecycle } from '@/api/task-lifecycle'
import { stageLabels, lifecycleStatusLabels } from '@/shared/lib/task-lifecycle-labels'
import { statusLabel } from '@/shared/lib/status-label'

export function IssueLifecycleSummary({
  lifecycle,
  legacyStatus,
}: {
  lifecycle?: TaskLifecycle['state'] | null
  legacyStatus?: string
}) {
  const { t } = useTranslation()
  if (!lifecycle)
    return legacyStatus ? <Badge tone="muted">{statusLabel(legacyStatus, t)}</Badge> : null
  return (
    <span className="inline-flex min-w-0 flex-wrap items-center gap-1">
      <Badge tone="muted">{stageLabels[lifecycle.stage]}</Badge>
      <Badge tone="muted">{lifecycleStatusLabels[lifecycle.status]}</Badge>
    </span>
  )
}
