import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'
import { LoginPage } from './'
import { useAuthStore } from '@/shared/auth/store'
import i18n from '@/shared/i18n/config'
import { SsoLogoutPendingError } from '@sdlc/ui/sso'

const beginSso = vi.hoisted(() => vi.fn<(...args: unknown[]) => Promise<void>>())
vi.mock('@sdlc/ui/sso', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@sdlc/ui/sso')>()),
  beginSso,
}))

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
  it('presents one platform login without product branding or legacy warnings', async () => {
    renderLogin('/login?logged_out=1')
    expect(await screen.findByRole('heading', { name: 'Вход в платформу', exact: true })).toBeInTheDocument()
    expect(screen.queryByText(/Base|SDLC|Task Tracker|Fleet Control|Wiki|CI[/]CD|Admin Panel|второй фактор|защита входа/i)).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Войти через SSO', exact: true })).toBeInTheDocument()
  })

  beforeEach(() => {
    beginSso.mockReset().mockImplementation(() => new Promise<void>(() => {}))
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
    expect(screen.getByRole('status')).toHaveTextContent('Переходим в Central Auth')
    expect(screen.getByRole('button', { name: 'Войти через SSO' })).toBeDisabled()
  })

  it('does not auto-login after global logout but allows a new explicit login', async () => {
    renderLogin('/login?logged_out=1')
    expect(beginSso).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('button', { name: 'Войти через SSO' }))
    expect(beginSso).toHaveBeenCalledTimes(1)
    expect(beginSso).toHaveBeenCalledWith(
      expect.objectContaining({ clientId: 'task-tracker' }),
      '/',
      { interactive: true },
    )
    expect(screen.getByRole('button', { name: 'Войти через SSO' })).toBeDisabled()
  })

  it('clears an error while retrying and disables repeat clicks', async () => {
    beginSso.mockRejectedValueOnce(new Error('private auth details'))
    renderLogin('/login?logged_out=1')
    await userEvent.click(screen.getByRole('button', { name: 'Войти через SSO' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Не удалось открыть Central Auth')
    expect(screen.queryByText(/private auth details/i)).not.toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: 'Повторить вход' }))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Войти через SSO' })).toBeDisabled()
    expect(beginSso).toHaveBeenCalledTimes(2)
  })

  it('localizes platform sign-in without legacy warnings in English', async () => {
    await i18n.changeLanguage('en')
    renderLogin('/login?logged_out=1')
    expect(screen.getByRole('heading', { name: 'Sign in to the platform' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Sign in with SSO' })).toBeEnabled()
    expect(screen.queryByText(/two-factor authentication is disabled/i)).not.toBeInTheDocument()
  })

  it.each([new SsoLogoutPendingError(), new DOMException('Cancelled', 'AbortError')])(
    'allows explicit retry after interrupted automatic navigation: %s',
    async (error) => {
      beginSso.mockRejectedValueOnce(error)
      renderLogin()
      const button = screen.getByRole('button', { name: 'Войти через SSO' })
      await waitFor(() => expect(button).toBeEnabled())
      expect(screen.queryByRole('alert')).not.toBeInTheDocument()
      expect(screen.queryByRole('status')).not.toBeInTheDocument()
      await userEvent.click(button)
      expect(beginSso).toHaveBeenLastCalledWith(
        expect.objectContaining({ clientId: 'task-tracker' }),
        '/',
        { interactive: true },
      )
      expect(button).toBeDisabled()
    },
  )

  it('releases the button when the legacy navigation promise completes', async () => {
    beginSso.mockResolvedValueOnce(undefined)
    renderLogin()
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Войти через SSO' })).toBeEnabled(),
    )
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })
  it('releases the explicit sign-in button when navigation is cancelled', async () => {
    beginSso.mockRejectedValueOnce(new DOMException('Cancelled', 'AbortError'))
    renderLogin('/login?logged_out=1')
    const button = screen.getByRole('button', { name: 'Войти через SSO' })
    await userEvent.click(button)
    await waitFor(() => expect(button).toBeEnabled())
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
    beginSso.mockImplementationOnce(() => new Promise(() => {}))
    await userEvent.click(button)
    expect(button).toBeDisabled()
    expect(beginSso).toHaveBeenCalledTimes(2)
  })
})
