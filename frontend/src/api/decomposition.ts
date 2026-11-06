import { api, type components } from './client'
export type PlanContent = components['schemas']['PlanContent']
export type PlanRevision = components['schemas']['PlanRevision']
export type SavePlan = components['schemas']['SavePlan']
export type PlanOperation = components['schemas']['PlanOperation']
export class PlanRequestError extends Error {
  constructor(public readonly status: number) {
    super('Не удалось выполнить операцию с планом')
  }
}
export async function readPlan(id: string, signal?: AbortSignal) {
  const { data, response } = await api.GET('/api/v1/issues/{id}/decomposition', {
    params: { path: { id } },
    signal,
  })
  if (!data) throw new PlanRequestError(response.status)
  return data
}
export async function savePlan(id: string, body: SavePlan) {
  const { data, response } = await api.POST('/api/v1/issues/{id}/decomposition', {
    params: { path: { id } },
    body,
  })
  if (!data) throw new PlanRequestError(response.status)
  return data
}
export async function readOperation(id: string, key: string) {
  const { data, response } = await api.GET('/api/v1/issues/{id}/decomposition/operations/{key}', {
    params: { path: { id, key } },
  })
  if (!data) throw new PlanRequestError(response.status)
  return data
}
export async function readHistory(id: string, before?: number, signal?: AbortSignal) {
  const { data, response } = await api.GET('/api/v1/issues/{id}/decomposition/revisions', {
    params: { path: { id }, query: { before, limit: 20 } },
    signal,
  })
  if (!data) throw new PlanRequestError(response.status)
  return data
}
export async function readRevision(id: string, revision: number, signal?: AbortSignal) {
  const { data, response } = await api.GET(
    '/api/v1/issues/{id}/decomposition/revisions/{revision}',
    { params: { path: { id, revision } }, signal },
  )
  if (!data) throw new PlanRequestError(response.status)
  return data
}
