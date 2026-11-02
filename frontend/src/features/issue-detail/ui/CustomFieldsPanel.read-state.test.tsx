import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ApiError } from '@pdlc/ui/lib'
import { ThemeProvider } from '@pdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { CustomFieldsPanel } from './CustomFieldsPanel'

const mocks = vi.hoisted(() => ({
  fields: vi.fn(),
  values: vi.fn(),
  save: vi.fn(),
  mutate: vi.fn(),
}))
vi.mock('@/shared/api/hooks', () => ({
  useProjectCustomFields: mocks.fields,
  useIssueCustomFieldValues: mocks.values,
  useSetIssueCustomFieldValue: mocks.save,
}))
const field = {
  id: 'field-one',
  project_id: 'project-one',
  name: 'Customer',
  field_type: 'text',
  options: [],
  is_required: false,
  created_at: '2026-10-25T00:00:00Z',
}
const readFields = vi.fn()
const readValues = vi.fn()
function ready(data: unknown, refetch: () => unknown) {
  return { data, error: null, isPending: false, isLoading: false, refetch }
}
function panel(issueId = 'issue-one', projectKey = 'TEST') {
  return (
    <MemoryRouter>
      <ThemeProvider theme="light" onThemeChange={() => {}}>
        <CustomFieldsPanel issueId={issueId} projectKey={projectKey} />
      </ThemeProvider>
    </MemoryRouter>
  )
}
beforeEach(async () => {
  vi.clearAllMocks()
  await i18n.changeLanguage('en')
  mocks.fields.mockReturnValue(ready([field], readFields))
  mocks.values.mockReturnValue(ready([{ field_id: field.id, value: 'Confirmed' }], readValues))
  mocks.save.mockReturnValue({ mutate: mocks.mutate, isPending: false })
})
afterEach(cleanup)

it.each(['fields', 'values'] as const)(
  'does not call an unread %s resource empty or editable',
  (resource) => {
    mocks[resource].mockReturnValue({
      ...ready(undefined, resource === 'fields' ? readFields : readValues),
      isPending: true,
      isLoading: true,
    })
    render(panel())
    expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'true')
    expect(screen.queryByText(/no custom fields configured/i)).not.toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    expect(mocks.mutate).not.toHaveBeenCalled()
  },
)

it('shows a confirmed empty catalog without waiting for an unnecessary values resource', () => {
  mocks.fields.mockReturnValue(ready([], readFields))
  mocks.values.mockReturnValue({
    ...ready(undefined, readValues),
    isPending: true,
    isLoading: true,
  })
  render(panel())
  expect(screen.getByText(/no custom fields configured/i)).toBeInTheDocument()
  expect(screen.queryByRole('status')).not.toBeInTheDocument()
  expect(mocks.mutate).not.toHaveBeenCalled()
})

it.each([401, 403, 404, 503])(
  'shows an initial %s failure instead of empty definitions',
  (status) => {
    mocks.fields.mockReturnValue({
      ...ready(undefined, readFields),
      error: new ApiError(status, 'unavailable'),
    })
    render(panel())
    expect(screen.queryByText(/no custom fields configured/i)).not.toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    expect(mocks.mutate).not.toHaveBeenCalled()
    if (status === 503) {
      fireEvent.click(screen.getByRole('button', { name: /retry/i }))
      expect(readFields).toHaveBeenCalledOnce()
      expect(readValues).toHaveBeenCalledOnce()
    }
  },
)

it('does not replace unread values with blank editable inputs after an initial failure', () => {
  mocks.values.mockReturnValue({
    ...ready(undefined, readValues),
    error: new ApiError(503, 'unavailable'),
  })
  render(panel())
  expect(screen.getByRole('alert')).toBeInTheDocument()
  expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
  expect(mocks.mutate).not.toHaveBeenCalled()
})

it('retains a local draft through a temporary read failure and only saves after recovery', () => {
  const { rerender } = render(panel())
  const input = screen.getByRole('textbox', { name: 'Customer' })
  fireEvent.change(input, { target: { value: 'Unsaved local edit' } })
  mocks.values.mockReturnValue({
    ...ready([{ field_id: field.id, value: 'Confirmed' }], readValues),
    error: new ApiError(503, 'unavailable'),
  })
  rerender(panel())
  expect(screen.getByRole('status')).toBeInTheDocument()
  expect(screen.getByRole('textbox', { name: 'Customer' })).toBe(input)
  expect(input).toHaveValue('Unsaved local edit')
  expect(input).toBeDisabled()
  fireEvent.blur(input)
  expect(mocks.mutate).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: /retry/i }))
  expect(readFields).toHaveBeenCalledOnce()
  expect(readValues).toHaveBeenCalledOnce()
  mocks.values.mockReturnValue(
    ready([{ field_id: field.id, value: 'New server value' }], readValues),
  )
  rerender(panel())
  expect(screen.getByRole('textbox', { name: 'Customer' })).toBe(input)
  expect(input).toBeEnabled()
  expect(input).toHaveValue('Unsaved local edit')
  expect(mocks.mutate).not.toHaveBeenCalled()
  fireEvent.blur(input)
  expect(mocks.mutate).toHaveBeenCalledWith(
    { fieldId: field.id, value: 'Unsaved local edit' },
    expect.objectContaining({ onSuccess: expect.any(Function), onError: expect.any(Function) }),
  )
})

it.each([401, 403, 404])(
  'hides cached values on %s instead of offering stale editing',
  (status) => {
    mocks.values.mockReturnValue({
      ...ready([{ field_id: field.id, value: 'Private value' }], readValues),
      error: new ApiError(status, 'unavailable'),
    })
    render(panel())
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    expect(screen.queryByDisplayValue('Private value')).not.toBeInTheDocument()
    expect(mocks.mutate).not.toHaveBeenCalled()
  },
)

it('prioritizes loss of value access over a cached definition read failure', () => {
  mocks.fields.mockReturnValue({
    ...ready([field], readFields),
    error: new ApiError(503, 'unavailable'),
  })
  mocks.values.mockReturnValue({
    ...ready([{ field_id: field.id, value: 'Private value' }], readValues),
    error: new ApiError(403, 'denied'),
  })
  render(panel())
  expect(document.querySelector('[data-page-state="permission-denied"]')).not.toBeNull()
  expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
  expect(screen.queryByText('Private value')).not.toBeInTheDocument()
})

it.each([
  ['select', 'Low'],
  ['multi-select', ['Low']],
  ['number', 12],
  ['date', '2026-10-25'],
])('disables an already rendered %s control after a cached read failure', (fieldType, value) => {
  mocks.fields.mockReturnValue(
    ready([{ ...field, field_type: fieldType, options: ['Low', 'High'] }], readFields),
  )
  mocks.values.mockReturnValue(ready([{ field_id: field.id, value }], readValues))
  const { rerender } = render(panel())
  const control = screen.getByLabelText('Customer')
  expect(control).toBeEnabled()
  mocks.values.mockReturnValue({
    ...ready([{ field_id: field.id, value }], readValues),
    error: new ApiError(503, 'unavailable'),
  })
  rerender(panel())
  expect(screen.getByLabelText('Customer')).toBe(control)
  expect(control).toBeDisabled()
  fireEvent.blur(control)
  expect(mocks.mutate).not.toHaveBeenCalled()
})

it.each([
  ['issue-two', 'TEST'],
  ['issue-one', 'OTHER'],
])('does not transfer a draft to %s/%s', (issueId, projectKey) => {
  const { rerender } = render(panel())
  fireEvent.change(screen.getByRole('textbox', { name: 'Customer' }), {
    target: { value: 'Wrong issue draft' },
  })
  rerender(panel(issueId, projectKey))
  expect(screen.getByRole('textbox', { name: 'Customer' })).toHaveValue('Confirmed')
  expect(mocks.save).toHaveBeenLastCalledWith(issueId)
  expect(mocks.fields).toHaveBeenLastCalledWith(projectKey)
  expect(mocks.mutate).not.toHaveBeenCalled()
})
