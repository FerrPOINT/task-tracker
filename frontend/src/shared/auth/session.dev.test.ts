import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const sso = vi.hoisted(() => ({ restoreSso: vi.fn() }))
vi.mock('@pdlc/ui/sso', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@pdlc/ui/sso')>()),
  ...sso,
}))

beforeEach(() => {
  vi.resetModules()
  vi.clearAllMocks()
  vi.stubEnv('VITE_AUTH_ISSUER', 'http://localhost:8701')
  sso.restoreSso.mockRejectedValue(new Error('Login required'))
})
afterEach(() => vi.unstubAllEnvs())

describe.each(['sdlc', 'pdlc'])('deployment-owned SSO client in %s namespace', (namespace) => {
  it.each([
    { configured: undefined, expected: 'task-tracker' },
    { configured: 'task-tracker-dev', expected: 'task-tracker-dev' },
  ])('restores the session with $expected', async ({ configured, expected }) => {
    vi.stubEnv('VITE_AUTH_CLIENT_ID', configured)
    vi.stubEnv('VITE_AUTH_NAMESPACE', namespace)
    const { restoreTrackerSession } = await import('./session')
    await expect(restoreTrackerSession()).rejects.toThrow('Login required')
    expect(sso.restoreSso).toHaveBeenCalledWith({
      issuer: 'http://localhost:8701',
      clientId: expected,
      namespace,
    })
  })
})
