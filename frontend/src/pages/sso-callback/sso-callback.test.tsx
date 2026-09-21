import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router'
import { SsoCallbackPage } from './'
import { useAuthStore } from '@/shared/auth/store'
import i18n from '@/shared/i18n/config'

const completeSso = vi.hoisted(() => vi.fn())
vi.mock('@sdlc/ui/sso', () => ({ completeSso }))

function renderCallback() {
  return render(
    <MemoryRouter initialEntries={['/sso/callback']}>
      <Routes>
        <Route path="/sso/callback" element={<SsoCallbackPage />} />
        <Route path="/login" element={<p>Login destination</p>} />
        <Route path="/projects" element={<p>Projects destination</p>} />
      </Routes>
    </MemoryRouter>,
  )
}

describe('SsoCallbackPage', () => {
  beforeEach(() => {
    completeSso.mockReset()
    useAuthStore.getState().logout()
  })

  afterEach(async () => {
    vi.unstubAllGlobals()
    await i18n.changeLanguage('ru')
  })

  it('shows a localized error and a retry route without exposing exception text', async () => {
    completeSso.mockRejectedValueOnce(new Error('private token exchange details'))
    renderCallback()

    expect(await screen.findByRole('alert')).toHaveTextContent('Не удалось завершить вход')
    expect(screen.queryByText(/private token exchange details/i)).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Повторить вход' }))
    expect(screen.getByText('Login destination')).toBeInTheDocument()
  })

  it('does not store a token when the profile request fails', async () => {
    completeSso.mockResolvedValueOnce({ accessToken: 'test-token', returnTo: '/projects' })
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({ ok: false })),
    )
    renderCallback()

    expect(await screen.findByRole('alert')).toHaveTextContent('Не удалось завершить вход')
    expect(useAuthStore.getState().token).toBeNull()
  })

  it('shows pending state and navigates after the profile is confirmed', async () => {
    let resolveSession!: (session: { accessToken: string; returnTo: string }) => void
    completeSso.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveSession = resolve
      }),
    )
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        json: async () => ({
          id: 'user-1',
          email: 'qa@example.test',
          username: 'qa',
          display_name: 'QA User',
        }),
      })),
    )
    renderCallback()
    expect(screen.getByRole('status')).toHaveTextContent('Завершаем вход')
    resolveSession({ accessToken: 'test-token', returnTo: '/projects' })

    await waitFor(() => expect(screen.getByText('Projects destination')).toBeInTheDocument())
    expect(useAuthStore.getState().token).toBe('test-token')
  })

  it('localizes callback failure in English', async () => {
    await i18n.changeLanguage('en')
    completeSso.mockRejectedValueOnce(new Error('raw exception'))
    renderCallback()

    expect(await screen.findByRole('alert')).toHaveTextContent('Could not finish signing in')
    expect(screen.getByRole('button', { name: 'Try signing in again' })).toBeInTheDocument()
  })
})
