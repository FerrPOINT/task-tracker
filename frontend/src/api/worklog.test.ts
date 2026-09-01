import { beforeEach, describe, expect, it, vi } from 'vitest'
import { listWorklogs } from './worklog'

const GET = vi.hoisted(() => vi.fn())

vi.mock('./client', () => ({ api: { GET } }))

function worklogDto(
  index: number,
  startedAt = new Date(Date.UTC(2026, 8, 1, 0, 0, index)).toISOString(),
) {
  return {
    id: `worklog-${index}`,
    issue_id: 'issue-1',
    author_id: 'user-1',
    author_name: 'Demo User',
    started_at: startedAt,
    duration_seconds: 900,
    description: `Worklog ${index}`,
    created_at: startedAt,
    updated_at: startedAt,
  }
}

describe('worklog API wrapper', () => {
  beforeEach(() => {
    GET.mockReset()
  })

  it('loads every worklog page instead of stopping at the first API page', async () => {
    const firstPage = Array.from({ length: 500 }, (_, index) => worklogDto(index))
    const lastPageNewestFirst = worklogDto(500, '2026-09-02T00:00:00.000Z')
    GET.mockResolvedValueOnce({ data: { worklogs: firstPage } })
    GET.mockResolvedValueOnce({ data: { worklogs: [lastPageNewestFirst] } })

    const worklogs = await listWorklogs('issue-1')

    expect(worklogs).toHaveLength(501)
    expect(worklogs[0]?.id).toBe('worklog-500')
    expect(GET).toHaveBeenNthCalledWith(1, '/api/v1/issues/{issue_id}/worklogs', {
      params: {
        path: { issue_id: 'issue-1' },
        query: { limit: 500, offset: 0 },
      },
    })
    expect(GET).toHaveBeenNthCalledWith(2, '/api/v1/issues/{issue_id}/worklogs', {
      params: {
        path: { issue_id: 'issue-1' },
        query: { limit: 500, offset: 500 },
      },
    })
  })
})
