import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createIssue } from './issue-create'

const POST = vi.hoisted(() => vi.fn())
vi.mock('./client', () => ({ api: { POST } }))
const command = {
  project_key: 'TT',
  issue_type: 'task',
  priority: 'medium',
  summary: 'Issue',
  operation_id: 'operation',
}

describe('issue creation response classification', () => {
  beforeEach(() => POST.mockReset())

  it.each([400, 422])(
    'identifies a structured validation rejection (%s) without API details',
    async (status) => {
      POST.mockResolvedValue({
        response: new Response(null, { status }),
        error: { error: { code: 'VALIDATION_ERROR', message: 'private API details' } },
      })
      await expect(createIssue(command)).rejects.toMatchObject({
        name: 'IssueCreationValidationError',
        message: 'Failed to create issue',
      })
    },
  )

  it.each([
    { status: 400, error: { error: 'legacy details' } },
    { status: 422, error: { error: { code: 'OTHER_ERROR' } } },
    { status: 409, error: { error: { code: 'VALIDATION_ERROR' } } },
    { status: 503, error: { error: { code: 'VALIDATION_ERROR' } } },
  ])(
    'does not classify an uncertain response as a validation rejection: $status',
    async (reply) => {
      POST.mockResolvedValue({
        response: new Response(null, { status: reply.status }),
        error: reply.error,
      })
      await expect(createIssue(command)).rejects.toMatchObject({
        name: 'Error',
        message: 'Failed to create issue',
      })
    },
  )

  it('keeps the idempotency key out of the JSON payload', async () => {
    POST.mockResolvedValue({
      data: { id: 'created' },
      response: new Response(null, { status: 201 }),
    })
    await expect(createIssue(command)).resolves.toMatchObject({ id: 'created' })
    expect(POST).toHaveBeenCalledWith('/api/v1/issues', {
      body: { project_key: 'TT', issue_type: 'task', priority: 'medium', summary: 'Issue' },
      headers: { 'Idempotency-Key': 'operation' },
    })
  })
})
