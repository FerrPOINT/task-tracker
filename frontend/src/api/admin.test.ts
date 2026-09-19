import { beforeEach, describe, expect, it, vi } from 'vitest'
import { listAdminAuditLog, listAdminSettings, updateAdminSetting } from './admin'

const GET = vi.hoisted(() => vi.fn())
const PUT = vi.hoisted(() => vi.fn())

vi.mock('./client', () => ({ api: { GET, PUT } }))

describe('admin API wrapper', () => {
  beforeEach(() => vi.clearAllMocks())

  it('uses generated admin paths and unwraps list responses', async () => {
    GET.mockResolvedValueOnce({
      data: { settings: [{ key: 'instance.name' }] },
    }).mockResolvedValueOnce({ data: { entries: [{ id: 'a1' }] } })

    await expect(listAdminSettings()).resolves.toEqual([{ key: 'instance.name' }])
    await expect(listAdminAuditLog(25)).resolves.toEqual([{ id: 'a1' }])

    expect(GET).toHaveBeenNthCalledWith(1, '/api/v1/admin/system-settings')
    expect(GET).toHaveBeenNthCalledWith(2, '/api/v1/admin/audit-log', {
      params: { query: { limit: 25 } },
    })
  })

  it('sends typed setting updates to the admin endpoint', async () => {
    PUT.mockResolvedValue({ data: { key: 'instance.name' } })
    await updateAdminSetting({ key: 'instance.name', value: 'Tracker' })

    expect(PUT).toHaveBeenCalledWith('/api/v1/admin/system-settings', {
      body: { key: 'instance.name', value: 'Tracker' },
    })
  })

  it('throws a useful error when an admin request has no response data', async () => {
    GET.mockResolvedValue({ error: { message: 'forbidden' } })
    await expect(listAdminSettings()).rejects.toThrow('forbidden')
  })
})
