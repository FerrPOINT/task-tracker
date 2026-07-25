import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'

import { RegisterPage } from './'
import { useAuthStore } from '@/shared/auth/store'
import { ThemeProvider } from '@/shared/lib/theme'

const register = vi.hoisted(() => vi.fn())
vi.mock('@/api/auth', () => ({ register }))

function wrapper(children: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  return (
    <ThemeProvider>
      <QueryClientProvider client={qc}>
        <MemoryRouter>{children}</MemoryRouter>
      </QueryClientProvider>
    </ThemeProvider>
  )
}

describe('RegisterPage', () => {
  beforeEach(() => {
    useAuthStore.setState({ token: null, userId: null, email: null })
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('renders register form and submits', async () => {
    register.mockResolvedValueOnce({
      access_token: 'tok',
      user_id: 'u1',
      email: 'new@example.com',
    } as any)

    render(wrapper(<RegisterPage />))
    expect(screen.getByText('TaskTracker')).toBeInTheDocument()

    await userEvent.type(screen.getByLabelText(/имя пользователя|username/i), 'newuser')
    await userEvent.type(screen.getByLabelText(/email/i), 'new@example.com')
    await userEvent.type(screen.getByLabelText(/^пароль|^password/i), 'password123')
    await userEvent.type(screen.getByLabelText(/повтор пароля|repeat password/i), 'password123')

    const submit = screen.getByRole('button', { name: /зарегистрироваться|sign up/i })
    await userEvent.click(submit)

    await waitFor(() => expect(register).toHaveBeenCalled())
  })
})
