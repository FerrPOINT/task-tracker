import { z } from 'zod'
import { api } from './client'
import type { components } from './generated'

export type HandoffPreview = components['schemas']['TaskHandoffPreview']
export type HandoffCommand = components['schemas']['TaskHandoffCommand']
export type HandoffOperation = components['schemas']['TaskHandoffOperation']
export type HandoffCancel = components['schemas']['CancelTaskHandoffCommand']
export type HandoffDocument = components['schemas']['RequirementsDocument']
export type HandoffBlocker = components['schemas']['TaskExecutionBlocker']

const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
const operationKey = z
  .string()
  .min(1)
  .refine((value) => new TextEncoder().encode(value).length <= 128 && !/\p{Cc}/u.test(value))
const lines = z.array(z.string().trim().min(1))
// Input shape validation only. Stage eligibility and required technical material
// are decided by Tracker; the client does not implement a second transition graph.
export const handoffCommandSchema: z.ZodType<HandoffCommand> = z
  .object({
    expected_revision: revision,
    expected_routing_policy_version: revision,
    confirmed_issue_sha256: z.string().regex(/^[0-9a-f]{64}$/),
    idempotency_key: operationKey,
    requirements: z
      .object({
        goal: z.string().trim().min(1),
        scope: lines,
        exclusions: lines,
        scenarios: lines,
        acceptance_criteria: lines,
        constraints: lines,
        dependencies: lines,
        assumptions: lines,
        checklist: lines,
        prerequisites: lines,
      })
      .strict()
      .refine((doc) => new TextEncoder().encode(JSON.stringify(doc)).length <= 262144),
  })
  .strict()

const cancelSchema: z.ZodType<HandoffCancel> = z
  .object({ expected_revision: revision, idempotency_key: operationKey })
  .strict()

export class HandoffRequestError extends Error {
  constructor(readonly status: number) {
    super(
      status === 409
        ? 'Материалы или состояние задачи изменились. Проверьте актуальные данные.'
        : status === 403
          ? 'Недостаточно доступа для этой операции.'
          : status === 422
            ? 'Проверьте цель, рамки, сценарии, критерии приёмки, проверки и предусловия.'
            : status === 404
              ? 'Операция или задача не найдена.'
              : 'Не удалось получить подтверждение операции. Проверьте её результат перед повтором.',
    )
  }
}
export function handoffOutcomeUnknown(error: unknown): boolean {
  return (
    !(error instanceof HandoffRequestError) ||
    error.status >= 500 ||
    error.status < 400 ||
    error.status === 408
  )
}
export async function getHandoffPreview(id: string, signal?: AbortSignal): Promise<HandoffPreview> {
  const { data, error, response } = await api.GET('/api/v1/issues/{id}/handoff', {
    params: { path: { id } },
    signal,
  })
  if (error || !data) throw new HandoffRequestError(response.status)
  return data
}
export async function requestHandoff(id: string, input: HandoffCommand): Promise<HandoffOperation> {
  const body = handoffCommandSchema.parse(input)
  const { data, error, response } = await api.POST('/api/v1/issues/{id}/handoff', {
    params: { path: { id } },
    body,
  })
  if (error || !data) throw new HandoffRequestError(response.status)
  return data
}
export async function getHandoffOperation(
  id: string,
  key: string,
  signal?: AbortSignal,
): Promise<HandoffOperation> {
  const { data, error, response } = await api.GET('/api/v1/issues/{id}/handoff/operations/{key}', {
    params: { path: { id, key } },
    signal,
  })
  if (error || !data) throw new HandoffRequestError(response.status)
  return data
}
export async function cancelHandoff(id: string, handoff_id: string, body: HandoffCancel) {
  const { data, error, response } = await api.POST(
    '/api/v1/issues/{id}/handoff/{handoff_id}/cancel',
    { params: { path: { id, handoff_id } }, body: cancelSchema.parse(body) },
  )
  if (error || !data) throw new HandoffRequestError(response.status)
  return data
}
