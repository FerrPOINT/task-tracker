import { expect, describe, it, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { I18nextProvider } from 'react-i18next'
import i18n from '@/shared/i18n/config'
import { LogWorkDialog } from './LogWorkDialog'

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  i18n.changeLanguage('en')
  return (
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={qc}>{children}</QueryClientProvider>
    </I18nextProvider>
  )
}

describe('LogWorkDialog', () => {
  it('submits valid input', async () => {
    const onSubmit = vi.fn()
    render(
      <LogWorkDialog open={true} onOpenChange={() => {}} onSubmit={onSubmit} />,
      { wrapper },
    )

    fireEvent.change(screen.getByLabelText(/Time spent/i), {
      target: { value: '1h 30m' },
    })
    fireEvent.change(screen.getByLabelText(/Remaining estimate/i), {
      target: { value: '2h' },
    })
    fireEvent.change(screen.getByLabelText(/Started/i), {
      target: { value: '2026-07-20' },
    })
    fireEvent.change(screen.getByLabelText(/Comment/i), {
      target: { value: 'Testing' },
    })

    fireEvent.click(screen.getByText(/Save/i))

    await waitFor(() => expect(onSubmit).toHaveBeenCalled())
    const arg = onSubmit.mock.calls[0]![0]
    expect(arg.timeSpent).toBe('1h 30m')
    expect(arg.remainingEstimate).toBe('2h')
    expect(arg.comment).toBe('Testing')
  })

  it('shows validation error on empty time spent', async () => {
    const onSubmit = vi.fn()
    render(
      <LogWorkDialog open={true} onOpenChange={() => {}} onSubmit={onSubmit} />,
      { wrapper },
    )

    fireEvent.click(screen.getByText(/Save/i))

    await waitFor(() => {
      expect(screen.getByText(/Provide time spent/i)).toBeInTheDocument()
    })
    expect(onSubmit).not.toHaveBeenCalled()
  })
})
