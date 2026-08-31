import { describe, expect, it, vi } from 'vitest'

const createClient = vi.hoisted(() => vi.fn(() => ({ use: vi.fn() })))

vi.mock('openapi-fetch', () => ({ default: createClient }))
vi.mock('@/shared/auth/store', () => ({
  useAuthStore: { getState: vi.fn(() => ({ token: null, logout: vi.fn() })), setState: vi.fn() },
}))

describe('API client transport', () => {
  it('includes HttpOnly cookies when the API is cross-origin', async () => {
    await import('./client')
    expect(createClient).toHaveBeenCalledWith(
      expect.objectContaining({ credentials: 'include' }),
    )
  })
})
