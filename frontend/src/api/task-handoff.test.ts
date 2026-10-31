import { describe, expect, it, vi, beforeEach } from 'vitest'
const wire = vi.hoisted(() => {
  vi.stubEnv('VITE_API_BASE_URL', 'https://tracker.example.test/tracker')
  const fn = vi.fn()
  vi.stubGlobal('fetch', fn)
  return fn
})
import {
  requestHandoff,
  cancelHandoff,
  getHandoffOperation,
  handoffOutcomeUnknown,
  HandoffRequestError,
  type HandoffCommand,
} from './task-handoff'
function input(): HandoffCommand {
  return {
    expected_revision: 3,
    expected_routing_policy_version: 4,
    confirmed_issue_sha256: 'a'.repeat(64),
    idempotency_key: 'исходный-key',
    requirements: {
      goal: 'Реальная цель',
      scope: [],
      exclusions: [],
      scenarios: [],
      acceptance_criteria: [],
      constraints: [],
      dependencies: [],
      assumptions: [],
      checklist: [],
      prerequisites: [],
    },
  }
}
beforeEach(() => wire.mockReset())
describe('human handoff transport', () => {
  it('sends the original key, revision and human materials through the prefixed client', async () => {
    wire.mockResolvedValueOnce(Response.json({ execution_requested: true }))
    const command = input()
    await requestHandoff('issue', command)
    const request = wire.mock.calls[0]![0] as Request
    expect(new URL(request.url).pathname).toBe('/tracker/api/v1/issues/issue/handoff')
    expect(request.method).toBe('POST')
    expect(await request.json()).toEqual(command)
  })
  it('rejects invalid UTF-8 keys, control characters and injected authority before HTTP', async () => {
    for (const key of ['я'.repeat(65), 'bad\u0085key', ''])
      await expect(requestHandoff('issue', { ...input(), idempotency_key: key })).rejects.toThrow()
    await expect(requestHandoff('issue', { ...input(), expected_revision: 0 })).rejects.toThrow()
    await expect(
      requestHandoff('issue', Object.assign(input(), { stage: 'Deployment' })),
    ).rejects.toThrow()
    await expect(
      cancelHandoff('issue', 'handoff', { expected_revision: 1, idempotency_key: 'я'.repeat(65) }),
    ).rejects.toThrow()
    expect(wire).not.toHaveBeenCalled()
  })
  it('never retries a rejected command or replaces its original operation key', async () => {
    wire.mockResolvedValueOnce(new Response(null, { status: 409 }))
    await expect(requestHandoff('issue', input())).rejects.toMatchObject({ status: 409 })
    expect(wire).toHaveBeenCalledTimes(1)
    wire.mockResolvedValueOnce(Response.json({ execution_requested: true }))
    await getHandoffOperation('issue', 'original /?key')
    expect(new URL((wire.mock.calls[1]![0] as Request).url).pathname).toContain(
      '/operations/original%20%2F%3Fkey',
    )
  })
  it('distinguishes unknown transport outcomes from definitive rejections', () => {
    expect(handoffOutcomeUnknown(new TypeError('lost response'))).toBe(true)
    expect(handoffOutcomeUnknown(new HandoffRequestError(503))).toBe(true)
    expect(handoffOutcomeUnknown(new HandoffRequestError(409))).toBe(false)
    expect(handoffOutcomeUnknown(new HandoffRequestError(403))).toBe(false)
  })
})
