import { Badge } from '@pdlc/ui/ui'

const labels: Record<string, string> = {
  highest: 'Наивысший',
  high: 'Высокий',
  medium: 'Средний',
  low: 'Низкий',
  lowest: 'Наинизший',
}

/** Priority values and labels are Tracker rules; Base owns their visual indicator. */
export function PriorityBadge({ priority }: { priority: string }) {
  const normalized = priority.toLowerCase()
  const tone =
    normalized === 'high' || normalized === 'highest'
      ? 'danger'
      : normalized === 'medium'
        ? 'warning'
        : 'success'
  return (
    <Badge appearance="dot" tone={tone}>
      {labels[normalized] ?? priority}
    </Badge>
  )
}
