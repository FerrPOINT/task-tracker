import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter, Routes, Route } from 'react-router'
import { RequireAdministrator } from '@pdlc/ui/auth'
import { QueryClient } from '@tanstack/react-query'
import { setPlatformQueryClient } from '@pdlc/ui/platform'
import { RequireAuth } from './require-auth'
import { ssoConfig, useAuthStore } from './store'

const subject = '123e4567-e89b-12d3-a456-426614174000'
function decision(applications: string[], role = 'user', revision = 1) {
  const now = Date.now()
  return {
    schema_version: 1,
    issuer: ssoConfig.issuer,
    subject,
    role,
    applications,
    revision,
    enforced: true,
    observed_at: new Date(now).toISOString(),
    expires_at: new Date(now + 30000).toISOString(),
  }
}
function view() {
  return render(
    <MemoryRouter>
      <Routes>
        <Route element={<RequireAuth />}>
          <Route
            path="/"
            element={
              <>
                <p>Рабочий экран</p>
                <RequireAdministrator>
                  <button>Настроить систему</button>
                </RequireAdministrator>
              </>
            }
          />
        </Route>
      </Routes>
    </MemoryRouter>,
  )
}
beforeEach(() => {
  setPlatformQueryClient(new QueryClient())
  const payload = btoa(JSON.stringify({ sub: subject, exp: Math.floor(Date.now() / 1000) + 300 }))
    .replace(/=/g, '')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
  useAuthStore.getState().setAuth({
    token: 'header.' + payload + '.signature',
    userId: 'local-user',
    email: 'user@example.test',
  })
})
afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  useAuthStore.getState().logout()
})
it('admits assigned work without administrative UI', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => Response.json(decision(['task-tracker']))),
  )
  view()
  expect(await screen.findByText('Рабочий экран')).toBeInTheDocument()
  expect(screen.queryByRole('button', { name: 'Настроить систему' })).not.toBeInTheDocument()
  expect(
    vi.mocked(fetch).mock.calls.every(([url]) => String(url) === ssoConfig.issuer + '/auth/access'),
  ).toBe(true)
})
it('rejects a different application and an empty assignment without another SSO cycle', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => Response.json(decision(['another-app']))),
  )
  view()
  expect(await screen.findByText('Нет доступа')).toBeInTheDocument()
  expect(screen.queryByText('Рабочий экран')).not.toBeInTheDocument()
  expect(fetch).toHaveBeenCalledTimes(1)
})
it('retires the same-token work screen when the owner revokes its application', async () => {
  let proof = decision(['task-tracker'])
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => Response.json(proof)),
  )
  view()
  await screen.findByText('Рабочий экран')
  proof = decision([], 'user', 2)
  act(() => window.dispatchEvent(new Event('focus')))
  await waitFor(() => expect(screen.queryByText('Рабочий экран')).not.toBeInTheDocument())
  expect(screen.getByText('Нет доступа')).toBeInTheDocument()
})
it('uses the current administrator role even with an empty stored application set', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => Response.json(decision([], 'admin'))),
  )
  view()
  expect(await screen.findByRole('button', { name: 'Настроить систему' })).toBeInTheDocument()
})
