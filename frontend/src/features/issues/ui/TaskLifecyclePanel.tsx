import { TaskHandoffAction } from './TaskHandoffAction'
import { Badge, Button, ErrorState, LoadingState, toast } from '@pdlc/ui/ui'
import {
  useLifecycleTransition,
  useTaskLifecycle,
  useTaskLifecycleCatalog,
} from '../model/task-lifecycle'
import type { TaskLifecycle } from '@/api/task-lifecycle'
import {
  stageLabels,
  lifecycleStatusLabels,
  lifecycleActionLabel,
} from '@/shared/lib/task-lifecycle-labels'

export function TaskLifecycleActions({
  lifecycle,
  disabled = false,
}: {
  lifecycle: TaskLifecycle
  disabled?: boolean
}) {
  const change = useLifecycleTransition()
  return (
    <div className="flex flex-wrap items-center gap-2">
      {lifecycle.allowed_actions.map((option) => (
        <Button
          key={option.action}
          variant="outline"
          size="sm"
          disabled={disabled || change.isPending}
          onClick={() =>
            change.mutate(
              {
                id: lifecycle.task_id,
                command: {
                  action: option.action,
                  expected_revision: lifecycle.state.revision,
                  idempotency_key: crypto.randomUUID(),
                },
              },
              { onError: (error) => toast.error(error.message) },
            )
          }
        >
          {lifecycleActionLabel(option)}
        </Button>
      ))}
    </div>
  )
}
export function TaskLifecyclePanel({
  issueId,
  summary,
  disabled = false,
}: {
  issueId: string
  summary?: string
  disabled?: boolean
}) {
  const query = useTaskLifecycle(issueId)
  if (query.isPending) return <LoadingState message="Загружаем состояние задачи…" />
  if (!query.data)
    return <ErrorState message={query.error?.message ?? 'Состояние задачи недоступно'} />
  const value = query.data
  return (
    <section aria-label="Стадия и статус задачи" className="space-y-2">
      <div className="flex flex-wrap items-center gap-2">
        <Badge tone="muted">{stageLabels[value.state.stage]}</Badge>
        <Badge tone="muted">{lifecycleStatusLabels[value.state.status]}</Badge>
      </div>
      {value.draft_submode === 'clarification' && <Badge tone="muted">Уточнение</Badge>}
      {value.execution_busy && (
        <p className="text-xs text-text-muted">Ручные переходы временно недоступны.</p>
      )}
      <TaskLifecycleActions lifecycle={value} disabled={disabled} />
      <TaskHandoffAction issueId={issueId} summary={summary} disabled={disabled} />
    </section>
  )
}

export function TaskLifecycleStatusGate({
  issueId,
  summary,
  disabled = false,
}: {
  issueId: string
  summary?: string
  disabled?: boolean
}) {
  const catalog = useTaskLifecycleCatalog()
  if (catalog.isPending) return <LoadingState message="Загружаем модель задач…" />
  if (!catalog.data)
    return <ErrorState message={catalog.error?.message ?? 'Модель задач недоступна'} />
  return catalog.data.enabled ? (
    <TaskLifecyclePanel issueId={issueId} summary={summary} disabled={disabled} />
  ) : (
    <ErrorState message="Модель задач обновляется. Изменение стадии временно недоступно." />
  )
}
