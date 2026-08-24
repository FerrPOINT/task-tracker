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

export async function getVelocityReport(
  projectId: string,
  count = 6,
): Promise<VelocityResponse> {
  const { data, error } = await api.GET('/api/v1/reports/velocity', {
    params: { query: { projectId, count } },
  })
  if (error || !data) throw new Error('failed to load velocity report')
  return data
}

export async function getBurndownReport(sprintId: string): Promise<BurndownResponse> {
  const { data, error } = await api.GET('/api/v1/reports/burndown', {
    params: { query: { sprintId } },
  })
  if (error || !data) throw new Error('failed to load burndown report')
  return data
}

export async function getCumulativeFlowReport(
  projectId: string,
): Promise<CumulativeFlowResponse> {
  const { data, error } = await api.GET('/api/v1/reports/cumulative-flow', {
    params: { query: { projectId } },
  })
  if (error || !data) throw new Error('failed to load cumulative flow report')
  return data
}

export async function getControlChartReport(
  projectId: string,
): Promise<ControlChartResponse> {
  const { data, error } = await api.GET('/api/v1/reports/control-chart', {
    params: { query: { projectId } },
  })
  if (error || !data) throw new Error('failed to load control chart report')
  return data
}