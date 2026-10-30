import { ApiError, InvalidResponseError } from '@pdlc/ui/lib'
import { api } from './client'
import type { components } from './generated'

export type VelocityResponse = components['schemas']['VelocityResponse']
export type VelocitySprint = components['schemas']['VelocitySprintResponse']
export type BurndownResponse = components['schemas']['BurndownResponse']
export type BurndownPoint = components['schemas']['BurndownPointResponse']
export type CumulativeFlowResponse = components['schemas']['CumulativeFlowResponse']
export type CumulativeFlowPoint = components['schemas']['CumulativeFlowPointResponse']
export type ControlChartResponse = components['schemas']['ControlChartResponse']
export type ControlChartPoint = components['schemas']['ControlChartPointResponse']

export async function getVelocityReport(projectId: string, count = 6): Promise<VelocityResponse> {
  const { data, error, response } = await api.GET('/api/v1/reports/velocity', {
    params: { query: { project_id: projectId, count } },
  })
  if (error || !response.ok) throw new ApiError(response.status, 'failed to load velocity report')
  if (!data || !Array.isArray(data.sprints)) throw new InvalidResponseError(response.status)
  return data
}

export async function getBurndownReport(sprintId: string): Promise<BurndownResponse> {
  const { data, error, response } = await api.GET('/api/v1/reports/burndown', {
    params: { query: { sprint_id: sprintId } },
  })
  if (error || !response.ok) throw new ApiError(response.status, 'failed to load burndown report')
  if (!data || !Array.isArray(data.points)) throw new InvalidResponseError(response.status)
  return data
}

export async function getCumulativeFlowReport(projectId: string): Promise<CumulativeFlowResponse> {
  const { data, error, response } = await api.GET('/api/v1/reports/cumulative-flow', {
    params: { query: { project_id: projectId } },
  })
  if (error || !response.ok)
    throw new ApiError(response.status, 'failed to load cumulative flow report')
  if (!data || !Array.isArray(data.points)) throw new InvalidResponseError(response.status)
  return data
}

export async function getControlChartReport(projectId: string): Promise<ControlChartResponse> {
  const { data, error, response } = await api.GET('/api/v1/reports/control-chart', {
    params: { query: { project_id: projectId } },
  })
  if (error || !response.ok)
    throw new ApiError(response.status, 'failed to load control chart report')
  if (!data || !Array.isArray(data.points)) throw new InvalidResponseError(response.status)
  return data
}
