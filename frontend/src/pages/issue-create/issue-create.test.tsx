import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter, Routes, Route } from 'react-router'

import { IssueCreatePage } from './'
import { ThemeProvider } from '@/shared/lib/theme'
import { useAuthStore } from '@/shared/auth/store'

const createIssue = vi.hoisted(() => vi.fn(() => Promise.resolve({ id: 'new' })))
vi.mock('@/api/issue-create', () => ({
  createIssue,
}))

function wrapper(children: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  return (
    <ThemeProvider>
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={['/issues/create']}>
          <Routes>
            <Route path="/issues/create" element={children} />
            <Route path="/projects/:key/backlog" element={<div>Backlog</div>} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>
    </ThemeProvider>
  )
}

describe('IssueCreatePage', () => {
  beforeEach(() => {
    useAuthStore.setState({ token: 'tok', userId: 'u1', email: 'a@b' })
  })

  it('creates issue and navigates', async () => {
    render(wrapper(<IssueCreatePage />))
    await waitFor(() => expect(screen.getByText('Создать задачу')).toBeInTheDocument())

    const summary = screen.getByPlaceholderText(/Краткое описание задачи/i) as HTMLInputElement
    await userEvent.clear(summary)
    await userEvent.type(summary, 'Test issue')

    const submit = screen.getByRole('button', { name: /^создать$/i })
    await userEvent.click(submit)

    await waitFor(() => expect(createIssue).toHaveBeenCalled())
  })
})
