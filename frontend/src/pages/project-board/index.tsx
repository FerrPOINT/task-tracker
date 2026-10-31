import { ErrorState, LoadingState } from '@pdlc/ui/ui'
import { useTaskLifecycleCatalog } from '@/features/issues/model/task-lifecycle'
import { TaskStageBoard } from './stage-board'

export function ProjectBoardPage() {
  const catalog = useTaskLifecycleCatalog()
  if (catalog.isPending) return <LoadingState message="Загружаем модель задач…" />
  if (!catalog.data)
    return <ErrorState message={catalog.error?.message ?? 'Модель задач недоступна'} />
  if (!catalog.data.enabled)
    return (
      <ErrorState message="Модель задач обновляется. Доска станет доступна после завершения переноса." />
    )
  return <TaskStageBoard />
}
