import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'
import { LoginPage } from './'
import { useAuthStore } from '@/shared/auth/store'

const beginSso = vi.hoisted(() => vi.fn(async () => {}))
vi.mock('@sdlc/ui/sso', () => ({ beginSso }))

function renderLogin(path = '/login') {
  return render(<ThemeProvider><MemoryRouter initialEntries={[path]}><LoginPage /></MemoryRouter></ThemeProvider>)
}

describe('LoginPage', () => {
  beforeEach(() => {
    beginSso.mockClear()
    useAuthStore.getState().logout()
  })

  it('starts the central authorization flow without a local password form', async () => {
    renderLogin()
    await waitFor(() => expect(beginSso).toHaveBeenCalledWith(
      expect.objectContaining({ clientId: 'task-tracker' }), '/',
    ))
    expect(screen.queryByLabelText(/пароль/i)).not.toBeInTheDocument()
    expect(screen.getByText(/второй фактор.*отключён/i)).toBeInTheDocument()
  })

  it('does not auto-login after global logout but allows a new explicit login', async () => {
    renderLogin('/login?logged_out=1')
    expect(beginSso).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('button', { name: 'Войти через SDLC' }))
    expect(beginSso).toHaveBeenCalledTimes(1)
  })
})
