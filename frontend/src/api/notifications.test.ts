import { beforeEach, describe, expect, it, vi } from 'vitest'
import { listNotifications } from './notifications'

const GET = vi.hoisted(() => vi.fn())

vi.mock('./client', () => ({ api: { GET } }))

describe('notification API wrapper', () => {
  beforeEach(() => {
    GET.mockReset()
  })

  it('passes list query options and preserves unread_count', async () => {
    const response = {
      notifications: [
        {
          id: 'notification-1',
          event_type: 'issue_assigned',
          entity_type: 'issue',
          entity_id: 'issue-1',
          actor_id: null,
          title: 'Assigned to you',
          body: null,
          is_read: false,
          action_url: '/issues/TT-1',
          metadata: null,
          created_at: '2026-09-01T10:00:00Z',
        },
      ],
      unread_count: 12,
    }
    GET.mockResolvedValueOnce({ data: response })

    await expect(listNotifications({ includeRead: true, limit: 50, offset: 10 })).resolves.toEqual(
      response,
    )
    expect(GET).toHaveBeenCalledWith('/api/v1/notifications', {
      params: { query: { include_read: true, limit: 50, offset: 10 } },
    })
  })
})
