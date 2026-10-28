import { screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'

const run = vi.hoisted(() => ({ completions: 0, root: null as { unmount: () => void } | null }))
const subject = 'new-browser-user'
const accessToken = `header.${btoa(JSON.stringify({ sub: subject, exp: 4102444800 }))}.signature`

vi.mock('react-dom/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-dom/client')>()
  return {
    ...actual,
    createRoot: (...args: Parameters<typeof actual.createRoot>) => {
      run.root = actual.createRoot(...args)
      return run.root
    },
  }
})
vi.mock('@/widgets/app-shell', async () => {
  const { Outlet } = await import('react-router')
  return { AppShell: () => <Outlet /> }
})
vi.mock('@/pages/dashboard', () => ({ DashboardPage: () => <h1>Authenticated Tracker</h1> }))
vi.mock('@pdlc/ui/sso', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@pdlc/ui/sso')>()
  return {
    ...actual,
    completeSso: async () => {
      run.completions += 1
      if (run.completions !== 1) throw new Error('The one-use callback was already consumed')
      window.history.replaceState(null, '', '/sso/callback')
      return {
        accessToken,
        subject,
        email: 'browser@example.test',
        name: 'Browser user',
        expiresAt: Date.now() + 300000,
        returnTo: '/',
      }
    },
  }
})

afterEach(() => {
  run.root?.unmount()
  document.body.replaceChildren()
  vi.unstubAllGlobals()
})

// Importing the real application boots its module graph on the two-CPU QA runner.
it('finishes an interactive one-use SSO callback before retiring the private application tree', async () => {
  window.history.replaceState(null, '', '/sso/callback?code=one-use&state=fixture')
  document.body.innerHTML = '<div id="root"></div>'
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string) => {
      if (url.endsWith('/users/me'))
        return Response.json({
          id: subject,
          email: 'browser@example.test',
          username: 'browser',
          display_name: 'Browser user',
        })
      if (url.endsWith('/auth/access'))
        return Response.json(
          {
            schema_version: 1,
            issuer: 'http://localhost:7701',
            subject,
            role: 'user',
            applications: ['task-tracker'],
            revision: 1,
            enforced: true,
            observed_at: new Date().toISOString(),
            expires_at: new Date(Date.now() + 30000).toISOString(),
          },
          { headers: { 'Cache-Control': 'no-store' } },
        )
      throw new Error(`Unexpected request: ${url}`)
    }),
  )
  await import('../main')
  await waitFor(() =>
    expect(screen.getByRole('heading', { name: 'Authenticated Tracker' })).toBeVisible(),
  )
  expect(run.completions).toBe(1)
  expect(window.location.pathname).toBe('/')
}, 15_000)
