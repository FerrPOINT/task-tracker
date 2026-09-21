import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'
import { LoginPage } from './'
import { useAuthStore } from '@/shared/auth/store'
import i18n from '@/shared/i18n/config'

const beginSso = vi.hoisted(() => vi.fn(async () => {}))
vi.mock('@sdlc/ui/sso', () => ({ beginSso }))

function renderLogin(path = '/login') {
  return render(
    <ThemeProvider>
      <MemoryRouter initialEntries={[path]}>
        <LoginPage />
      </MemoryRouter>
    </ThemeProvider>,
  )
}

describe('LoginPage', () => {
  beforeEach(() => {
    beginSso.mockClear()
    useAuthStore.getState().logout()
  })

  afterEach(async () => {
    await i18n.changeLanguage('ru')
  })

  it('starts the central authorization flow without a local password form', async () => {
    renderLogin()
    await waitFor(() =>
      expect(beginSso).toHaveBeenCalledWith(
        expect.objectContaining({ clientId: 'task-tracker' }),
        '/',
      ),
    )
    expect(screen.queryByLabelText(/пароль/i)).not.toBeInTheDocument()
    expect(screen.getByText(/второй фактор.*отключён/i)).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent('Переходим в Central Auth')
    expect(screen.getByRole('button', { name: 'Войти через SDLC' })).toBeDisabled()
  })

  it('does not auto-login after global logout but allows a new explicit login', async () => {
    renderLogin('/login?logged_out=1')
    expect(beginSso).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('button', { name: 'Войти через SDLC' }))
    expect(beginSso).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('button', { name: 'Войти через SDLC' })).toBeDisabled()
  })

  it('clears an error while retrying and disables repeat clicks', async () => {
    beginSso.mockRejectedValueOnce(new Error('private auth details'))
    renderLogin('/login?logged_out=1')
    await userEvent.click(screen.getByRole('button', { name: 'Войти через SDLC' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Не удалось открыть Central Auth')
    expect(screen.queryByText(/private auth details/i)).not.toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: 'Повторить вход' }))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Войти через SDLC' })).toBeDisabled()
    expect(beginSso).toHaveBeenCalledTimes(2)
  })

  it('localizes sign-in and the required security notice in English', async () => {
    await i18n.changeLanguage('en')
    renderLogin('/login?logged_out=1')
    expect(screen.getByRole('heading', { name: 'Sign in to Task Tracker' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Sign in with SDLC' })).toBeEnabled()
    expect(screen.getByText(/two-factor authentication is disabled/i)).toBeInTheDocument()
  })
})
