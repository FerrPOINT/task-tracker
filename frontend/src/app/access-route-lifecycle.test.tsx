import { act, fireEvent, screen, waitFor } from '@testing-library/react'
import { afterAll, beforeAll, beforeEach, expect, it, vi } from 'vitest'
import type { Root } from 'react-dom/client'
import type { QueryClient } from '@tanstack/react-query'
import { useAuthStore, ssoConfig } from '@/shared/auth/store'
import { useSpaceSession } from '@/shared/auth/space-session'

const current = vi.hoisted(() => ({
  root: null as Root | null,
  client: null as QueryClient | null,
}))
vi.mock('react-dom/client', async (original) => {
  const module = await original<typeof import('react-dom/client')>()
  return {
    ...module,
    createRoot: (...args: Parameters<typeof module.createRoot>) => {
      current.root = module.createRoot(...args)
      return current.root
    },
  }
})
vi.mock('@/pages/sso-callback', () => ({
  SsoCallbackPage: () => <input aria-label="Public callback state" defaultValue="" />,
}))
vi.mock('@/pages/login', () => ({ LoginPage: () => <p>Public sign-in</p> }))
vi.mock('@/widgets/app-shell', async () => {
  const { useQueryClient } = await import('@tanstack/react-query')
  return {
    AppShell: () => {
      current.client = useQueryClient()
      return <input aria-label="Private draft" defaultValue="" />
    },
  }
})
vi.mock('@/shared/auth/session', async () => {
  const { SsoLoginRequiredError } = await import('@pdlc/ui/sso')
  return {
    restoreTrackerSession: vi
      .fn()
      .mockImplementation(() => Promise.reject(new SsoLoginRequiredError())),
  }
})
const first = '123e4567-e89b-12d3-a456-426614174000'
const second = '123e4567-e89b-12d3-a456-426614174001'
let router: (typeof import('./router'))['router']
function setSession(subject: string) {
  const token = `header.${btoa(JSON.stringify({ sub: subject, exp: Math.floor(Date.now() / 1000) + 300 }))}.signature`
  useAuthStore.getState().setAuth({ token, userId: subject, email: 'fixture@example.test' })
  useSpaceSession.setState({
    session: {
      identity: { issuer: ssoConfig.issuer, subject },
      userId: subject,
      expiresAt: Date.now() + 300000,
      isSystemAdmin: true,
      policyRevision: null,
      policyEpoch: 0,
      contextKey: null,
      contextEpoch: 0,
    },
  })
}
beforeAll(async () => {
  vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
  const container = document.createElement('div')
  container.id = 'root'
  document.body.append(container)
  window.history.replaceState(null, '', '/sso/callback')
  await act(async () => {
    await import('../main')
    router = (await import('./router')).router
  })
})
beforeEach(async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (_url, init?: RequestInit) => {
      const authorization = new Headers(init?.headers).get('Authorization')
      if (!authorization?.startsWith('Bearer ')) throw new Error('Missing fixture bearer')
      const payload = authorization.slice(7).split('.')[1]
      if (!payload) throw new Error('Missing fixture identity')
      const subject = JSON.parse(atob(payload)).sub
      const now = Date.now()
      return Response.json({
        schema_version: 1,
        issuer: ssoConfig.issuer,
        subject,
        role: 'admin',
        applications: ['task-tracker'],
        revision: 1,
        enforced: true,
        observed_at: new Date(now).toISOString(),
        expires_at: new Date(now + 30000).toISOString(),
      })
    }),
  )
  await act(async () => {
    useAuthStore.getState().logout()
    await router.navigate('/sso/callback')
  })
})
afterAll(async () => {
  await act(async () => current.root?.unmount())
  router.dispose()
  document.getElementById('root')?.remove()
  vi.unstubAllGlobals()
  vi.unstubAllEnvs()
})
it('keeps the public callback mounted when accepting the session', async () => {
  const callback = await screen.findByRole('textbox', { name: 'Public callback state' })
  fireEvent.change(callback, { target: { value: 'one-time-exchange' } })
  await act(async () => setSession(first))
  await waitFor(() =>
    expect(screen.getByRole('textbox', { name: 'Public callback state' })).toHaveValue(
      'one-time-exchange',
    ),
  )
  expect(screen.getByRole('textbox', { name: 'Public callback state' })).toBe(callback)
})
it('still clears protected forms and queries on account change and logout', async () => {
  await act(async () => {
    setSession(first)
    await router.navigate('/')
  })
  const draft = await screen.findByRole('textbox', { name: 'Private draft' })
  fireEvent.change(draft, { target: { value: 'first-user-only' } })
  const previous = current.client!
  previous.setQueryData(['private-fixture'], 'first-user-only')
  await act(async () => setSession(second))
  await waitFor(() =>
    expect(screen.getByRole('textbox', { name: 'Private draft' })).toHaveValue(''),
  )
  expect(previous.getQueryData(['private-fixture'])).toBeUndefined()
  expect(current.client!.getQueryData(['private-fixture'])).toBeUndefined()
  await act(async () => useAuthStore.getState().logout())
  expect(await screen.findByText('Public sign-in')).toBeVisible()
  expect(screen.queryByRole('textbox', { name: 'Private draft' })).not.toBeInTheDocument()
})
