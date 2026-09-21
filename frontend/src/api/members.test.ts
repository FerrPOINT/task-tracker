import { beforeEach, describe, expect, it, vi } from 'vitest'
import { listProjectMembers } from './members'

const GET = vi.hoisted(() => vi.fn())
vi.mock('./client', () => ({ api: { GET } }))

describe('project members API wrapper', () => {
  beforeEach(() => GET.mockReset())
  it('loads members for the requested project key', async () => {
    const response = { members: [{ project_id: 'p1', user_id: 'u2', role: 'member' }] }
    GET.mockResolvedValueOnce({ data: response })
    await expect(listProjectMembers('TT')).resolves.toEqual(response)
    expect(GET).toHaveBeenCalledWith('/api/v1/projects/{project_key}/members', {
      params: { path: { project_key: 'TT' } },
    })
  })
})
