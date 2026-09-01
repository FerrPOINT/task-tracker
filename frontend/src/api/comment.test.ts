import { beforeEach, describe, expect, it, vi } from 'vitest'
import { listComments } from './comment'

const GET = vi.hoisted(() => vi.fn())

vi.mock('./client', () => ({ api: { GET } }))

function commentDto(
  index: number,
  createdAt = new Date(Date.UTC(2026, 8, 1, 0, 0, index)).toISOString(),
) {
  return {
    id: `comment-${index}`,
    issue_id: 'issue-1',
    author_id: 'user-1',
    author_name: 'Demo User',
    body: `Comment ${index}`,
    created_at: createdAt,
    updated_at: createdAt,
  }
}

describe('comment API wrapper', () => {
  beforeEach(() => {
    GET.mockReset()
  })

  it('loads every comment page instead of stopping at the first API page', async () => {
    const firstPage = Array.from({ length: 500 }, (_, index) => commentDto(index))
    const lastPageNewestFirst = commentDto(500, '2026-09-02T00:00:00.000Z')
    GET.mockResolvedValueOnce({ data: { comments: firstPage } })
    GET.mockResolvedValueOnce({ data: { comments: [lastPageNewestFirst] } })

    const comments = await listComments('issue-1')

    expect(comments).toHaveLength(501)
    expect(comments[0]?.id).toBe('comment-500')
    expect(GET).toHaveBeenNthCalledWith(1, '/api/v1/issues/{issue_id}/comments', {
      params: {
        path: { issue_id: 'issue-1' },
        query: { limit: 500, offset: 0 },
      },
    })
    expect(GET).toHaveBeenNthCalledWith(2, '/api/v1/issues/{issue_id}/comments', {
      params: {
        path: { issue_id: 'issue-1' },
        query: { limit: 500, offset: 500 },
      },
    })
  })
})
