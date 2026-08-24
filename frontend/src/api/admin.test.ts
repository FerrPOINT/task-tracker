import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  createAdminUser,
  listAdminAuditLog,
  listAdminSettings,
  listAdminUsers,
  updateAdminSetting,
  updateAdminUserStatus,
} from './admin'

const GET = vi.hoisted(() => vi.fn())
const POST = vi.hoisted(() => vi.fn())
const PUT = vi.hoisted(() => vi.fn())

vi.mock('./client', () => ({ api: { GET, POST, PUT } }))

describe('admin API wrapper', () => {
  beforeEach(() => vi.clearAllMocks())

  it('uses generated admin paths and unwraps list responses', async () => {
    GET.mockResolvedValueOnce({ data: { users: [{ id: 'u1' }] } })
      .mockResolvedValueOnce({ data: { settings: [{ key: 'instance.name' }] } })
      .mockResolvedValueOnce({ data: { entries: [{ id: 'a1' }] } })

    await expect(listAdminUsers()).resolves.toEqual([{ id: 'u1' }])
    await expect(listAdminSettings()).resolves.toEqual([{ key: 'instance.name' }])
    await expect(listAdminAuditLog(25)).resolves.toEqual([{ id: 'a1' }])

    expect(GET).toHaveBeenNthCalledWith(1, '/api/v1/admin/users')
    expect(GET).toHaveBeenNthCalledWith(2, '/api/v1/admin/system-settings')
    expect(GET).toHaveBeenNthCalledWith(3, '/api/v1/admin/audit-log', {
      params: { query: { limit: 25 } },
    })
  })

  it('sends typed create, status, and setting updates to the admin endpoints', async () => {
    POST.mockResolvedValue({ data: { id: 'u2' } })
    PUT.mockResolvedValueOnce({}).mockResolvedValueOnce({ data: { key: 'instance.name' } })

    await createAdminUser({
      email: 'new@example.test',
      username: 'new',
      display_name: 'New',
      password: 'secret',
    })
    await updateAdminUserStatus({ id: 'u2', is_active: false })
    await updateAdminSetting({ key: 'instance.name', value: 'Tracker' })

    expect(POST).toHaveBeenCalledWith('/api/v1/admin/users', {
      body: { email: 'new@example.test', username: 'new', display_name: 'New', password: 'secret' },
    })
    expect(PUT).toHaveBeenNthCalledWith(1, '/api/v1/admin/users/{id}/status', {
      params: { path: { id: 'u2' } },
      body: { is_active: false },
    })
    expect(PUT).toHaveBeenNthCalledWith(2, '/api/v1/admin/system-settings', {
      body: { key: 'instance.name', value: 'Tracker' },
    })
  })

  it('throws a useful error when an admin request has no response data', async () => {
    GET.mockResolvedValue({ error: { message: 'forbidden' } })
    await expect(listAdminUsers()).rejects.toThrow('forbidden')
  })
})
