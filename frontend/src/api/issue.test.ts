import { beforeEach, describe, expect, it, vi } from 'vitest'
import { getIssue } from './issue'

const GET = vi.hoisted(() => vi.fn())

vi.mock('./client', () => ({ api: { GET } }))

describe('getIssue', () => {
  beforeEach(() => GET.mockReset())

  it('returns a loaded issue', async () => {
    const issue = {
      id: 'i1',
      key: 'TT-1',
      lifecycle: { issue_id: 'i1', enrolled: false, stage: 'Draft' },
    }
    GET.mockResolvedValue({ data: issue, response: { status: 200 } })

    await expect(getIssue('i1')).resolves.toEqual(issue)
  })

  it.each([
    undefined,
    { issue_id: 'foreign', enrolled: true, stage: 'Draft' },
    { issue_id: 'i1', stage: 'Draft' },
    { issue_id: 'i1', enrolled: 'false', stage: 'Draft' },
    { issue_id: 'i1', enrolled: false },
    { issue_id: 'i1', enrolled: true, stage: 'Unsupported' },
    { issue_id: 'i1', enrolled: true, stage: 'Draft', status: 'Done' },
    { issue_id: 'i1', enrolled: false, stage: 'Development' },
  ])('rejects missing or malformed lifecycle metadata: %j', async (lifecycle) => {
    GET.mockResolvedValue({ data: { id: 'i1', lifecycle }, response: { status: 200 } })
    await expect(getIssue('i1')).rejects.toThrow('Invalid issue lifecycle response')
  })

  it('returns null for a genuinely missing issue', async () => {
    GET.mockResolvedValue({ error: { message: 'not found' }, response: { status: 404 } })

    await expect(getIssue('missing')).resolves.toBeNull()
  })

  it('propagates server failures instead of reporting a missing issue', async () => {
    GET.mockResolvedValue({ error: { message: 'unavailable' }, response: { status: 503 } })

    await expect(getIssue('i1')).rejects.toThrow('Failed to load issue')
  })
})
