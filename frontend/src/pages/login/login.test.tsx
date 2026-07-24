import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'

import { LoginPage } from './'
import { useAuthStore } from '@/shared/auth/store'
import { ThemeProvider } from '@/shared/lib/theme'

const login = vi.hoisted(() => vi.fn())
vi.mock('@/api/auth', () => ({ login }))

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

describe('LoginPage', () => {
  beforeEach(() => {
    useAuthStore.setState({ token: null, userId: null, email: null })
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('renders login form and submits', async () => {
    login.mockResolvedValueOnce({
      access_token: 'tok',
      user_id: 'u1',
      email: 'demo@example.com',
    } as any)

    render(wrapper(<LoginPage />))
    expect(screen.getByText('TaskTracker')).toBeInTheDocument()

    const email = screen.getByDisplayValue('demo@example.com') as HTMLInputElement
    await userEvent.clear(email)
    await userEvent.type(email, 'demo@example.com')
    const password = screen.getByDisplayValue('demo') as HTMLInputElement
    await userEvent.clear(password)
    await userEvent.type(password, 'demo')

    const submit = screen.getByRole('button', { name: /войти/i })
    await userEvent.click(submit)

    await waitFor(() => expect(login).toHaveBeenCalled())
  })
})
