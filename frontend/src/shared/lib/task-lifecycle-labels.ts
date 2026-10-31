import type { LifecycleOption, TaskLifecycle, TaskStage } from '@/api/task-lifecycle'

export const stageLabels: Record<TaskStage, string> = {
  Draft: 'Черновик',
  Backlog: 'Бэклог',
  Analysis: 'Анализ',
  Architecture: 'Архитектура',
  Development: 'Разработка',
  Review: 'Ревью',
  Testing: 'Тестирование',
  Rework: 'Доработка',
  Deployment: 'Развёртывание',
}
export const lifecycleStatusLabels: Record<TaskLifecycle['state']['status'], string> = {
  Ready: 'Готова к работе',
  InProgress: 'В работе',
  Completed: 'Завершена',
}
const actionLabels = {
  start: 'Начать этап',
  complete: 'Завершить этап',
  request_rework: 'На доработку',
  reopen: 'Открыть повторно',
}
export function lifecycleActionLabel(option: LifecycleOption): string {
  return actionLabels[option.action]
}
