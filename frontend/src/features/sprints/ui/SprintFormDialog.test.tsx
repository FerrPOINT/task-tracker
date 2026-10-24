import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@pdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { SprintFormDialog } from './SprintFormDialog'
import type { Sprint } from '@/api/sprint'

beforeAll(() => {
  i18n.changeLanguage('en')
})

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

describe('SprintFormDialog', () => {
  it('associates an invalid date range with the end date and submits only after correction', () => {
    const onSubmit = vi.fn()
    render(
      wrapper(
        <SprintFormDialog open onOpenChange={vi.fn()} onSubmit={onSubmit} isPending={false} />,
      ),
    )
    fireEvent.change(screen.getByLabelText(/^name/i), { target: { value: 'Date check' } })
    fireEvent.change(screen.getByLabelText(/start date/i), { target: { value: '2026-10-24' } })
    const end = screen.getByLabelText(/end date/i)
    fireEvent.change(end, { target: { value: '2026-10-23' } })
    fireEvent.click(screen.getByRole('button', { name: /create sprint/i }))
    expect(onSubmit).not.toHaveBeenCalled()
    expect(end).toHaveAttribute('aria-invalid', 'true')
    expect(end).toHaveAccessibleDescription('End date cannot be earlier than start date')
    expect(screen.getByRole('alert')).toHaveTextContent(
      'End date cannot be earlier than start date',
    )
    fireEvent.change(end, { target: { value: '2026-10-24' } })
    fireEvent.click(screen.getByRole('button', { name: /create sprint/i }))
    expect(onSubmit).toHaveBeenCalledExactlyOnceWith(
      expect.objectContaining({
        name: 'Date check',
        start_date: '2026-10-24T00:00:00+00:00',
        end_date: '2026-10-24T00:00:00+00:00',
      }),
    )
    expect(end).not.toHaveAttribute('aria-invalid')
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('announces a failed save while retaining entered fields without an automatic retry', () => {
    const onSubmit = vi.fn()
    const props = { open: true, onOpenChange: vi.fn(), onSubmit, isPending: false }
    const view = render(wrapper(<SprintFormDialog {...props} />))
    fireEvent.change(screen.getByLabelText(/^name/i), { target: { value: 'Retained draft' } })
    view.rerender(wrapper(<SprintFormDialog {...props} error={new Error('Save not confirmed')} />))
    expect(screen.getByRole('alert')).toHaveTextContent('Save not confirmed')
    expect(screen.getByLabelText(/^name/i)).toHaveValue('Retained draft')
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('renders form fields when open', () => {
    render(
      wrapper(
        <SprintFormDialog
          open={true}
          onOpenChange={vi.fn()}
          onSubmit={vi.fn()}
          isPending={false}
        />,
      ),
    )
    expect(screen.getByPlaceholderText(/Sprint 1/i)).toBeInTheDocument()
    expect(
      screen.getByPlaceholderText(/what should be done by the end of the sprint/i),
    ).toBeInTheDocument()
  })

  it('submits form with entered values', () => {
    const onSubmit = vi.fn()
    render(
      wrapper(
        <SprintFormDialog
          open={true}
          onOpenChange={vi.fn()}
          onSubmit={onSubmit}
          isPending={false}
        />,
      ),
    )
    fireEvent.change(screen.getByPlaceholderText(/Sprint 1/i), { target: { value: 'Sprint 1' } })
    fireEvent.change(screen.getByPlaceholderText(/what should be done by the end of the sprint/i), {
      target: { value: 'Ship it' },
    })
    fireEvent.click(screen.getByRole('button', { name: /create sprint/i }))
    expect(onSubmit).toHaveBeenCalled()
    const arg = onSubmit.mock.calls[0]![0]
    expect(arg.name).toBe('Sprint 1')
    expect(arg.goal).toBe('Ship it')
  })

  it('shows edit title when sprint is provided', () => {
    const sprint: Sprint = {
      id: 'sp1',
      name: 'Sprint 1',
      goal: 'Ship it',
      issue_ids: [],
      state: 'planned',
      velocity: 0,
      start_date: '2026-08-01T00:00:00Z',
      end_date: '2026-08-14T00:00:00Z',
    }
    render(
      wrapper(
        <SprintFormDialog
          open={true}
          sprint={sprint}
          onOpenChange={vi.fn()}
          onSubmit={vi.fn()}
          isPending={false}
        />,
      ),
    )
    expect(screen.getByText(/edit sprint/i)).toBeInTheDocument()
    expect(screen.getByDisplayValue('Sprint 1')).toBeInTheDocument()
  })

  it('submits nulls when optional sprint fields are cleared in edit mode', () => {
    const onSubmit = vi.fn()
    const sprint: Sprint = {
      id: 'sp1',
      name: 'Sprint 1',
      goal: 'Ship it',
      issue_ids: [],
      state: 'planned',
      velocity: 0,
      start_date: '2026-08-01T00:00:00Z',
      end_date: '2026-08-14T00:00:00Z',
    }
    render(
      wrapper(
        <SprintFormDialog
          open={true}
          sprint={sprint}
          onOpenChange={vi.fn()}
          onSubmit={onSubmit}
          isPending={false}
        />,
      ),
    )

    fireEvent.change(screen.getByDisplayValue('Ship it'), { target: { value: '' } })
    fireEvent.change(screen.getByDisplayValue('2026-08-01'), { target: { value: '' } })
    fireEvent.change(screen.getByDisplayValue('2026-08-14'), { target: { value: '' } })
    fireEvent.click(screen.getByRole('button', { name: /save/i }))

    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({
        goal: null,
        start_date: null,
        end_date: null,
      }),
    )
  })
})
