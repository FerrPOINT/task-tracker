import { api } from './client'
import type { components } from './generated'

export type TaskLifecycle = components['schemas']['TaskLifecycleReadback']
export type LifecycleCommand = components['schemas']['TaskLifecycleTransitionCommand']
export type LifecycleOption = components['schemas']['TaskLifecycleTransitionOption']
export type TaskStage = components['schemas']['LifecycleStage']
export type StageBoard = components['schemas']['TaskBoard']

export async function getTaskLifecycle(id: string, signal?: AbortSignal): Promise<TaskLifecycle> {
  const { data, error } = await api.GET('/api/v1/issues/{id}/lifecycle', {
    params: { path: { id } },
    signal,
  })
  if (error || !data) throw new Error('Не удалось загрузить состояние задачи')
  return data
}
export async function transitionTaskLifecycle(
  id: string,
  command: LifecycleCommand,
): Promise<TaskLifecycle> {
  const { data, error, response } = await api.POST('/api/v1/issues/{id}/lifecycle/transitions', {
    params: { path: { id } },
    body: command,
  })
  if (error || !data)
    throw new Error(
      response.status === 409
        ? 'Состояние задачи изменилось или переход недоступен. Загружаем актуальные данные.'
        : 'Переход не выполнен. Проверьте доступ и обязательные материалы задачи.',
    )
  return data
}
export async function getStageBoard(projectKey: string, signal?: AbortSignal): Promise<StageBoard> {
  const { data, error } = await api.GET('/api/v2/projects/{project_key}/board', {
    params: { path: { project_key: projectKey } },
    signal,
  })
  if (error || !data) throw new Error('Не удалось загрузить стадии задач')
  return data
}

export async function getTaskLifecycleCatalog(signal?: AbortSignal) {
  const { data, error } = await api.GET('/api/v1/task-lifecycle', { signal })
  if (error || !data) throw new Error('Не удалось проверить модель задач')
  return data
}
