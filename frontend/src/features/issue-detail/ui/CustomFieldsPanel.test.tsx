import { describe, it, expect, vi, beforeAll, afterEach } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@/shared/lib/theme'
import i18n from '@/shared/i18n/config'
import { CustomFieldsPanel } from './CustomFieldsPanel'

beforeAll(() => {
  i18n.changeLanguage('en')
})

const mockFields = vi.hoisted(() => vi.fn())
const mockValues = vi.hoisted(() => vi.fn())
const mockSave = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useProjectCustomFields: (...args: unknown[]) => mockFields(...args),
  useIssueCustomFieldValues: (...args: unknown[]) => mockValues(...args),
  useSetIssueCustomFieldValue: () => ({ mutate: mockSave, isPending: false }),
}))

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

describe('CustomFieldsPanel', () => {
  afterEach(() => {
    mockFields.mockReset()
    mockValues.mockReset()
    mockSave.mockReset()
  })

  it('renders field labels and values', () => {
    mockFields.mockReturnValue({
      data: [
        {
          id: 'f1',
          project_id: 'p1',
          name: 'Priority',
          field_type: 'select',
          options: ['Low', 'Medium', 'High'],
          is_required: true,
          created_at: '2026-08-01T00:00:00Z',
        },
        {
          id: 'f2',
          project_id: 'p1',
          name: 'Estimate',
          field_type: 'number',
          options: [],
          is_required: false,
          created_at: '2026-08-01T00:00:00Z',
        },
      ],
      isLoading: false,
      error: null,
    })
    mockValues.mockReturnValue({
      data: [{ field_id: 'f1', value: 'High' }],
      isLoading: false,
      error: null,
    })
    render(wrapper(<CustomFieldsPanel issueId="i1" projectKey="TT" />))
    expect(screen.getByText(/Priority/)).toBeInTheDocument()
    expect(screen.getByText(/Estimate/)).toBeInTheDocument()
  })

  it('renders empty state when no custom fields', () => {
    mockFields.mockReturnValue({ data: [], isLoading: false, error: null })
    mockValues.mockReturnValue({ data: [], isLoading: false, error: null })
    render(wrapper(<CustomFieldsPanel issueId="i2" projectKey="TT" />))
    expect(screen.getByText(/no custom fields configured/i)).toBeInTheDocument()
  })

  it('formats date values for date inputs and saves date-only values', () => {
    mockFields.mockReturnValue({
      data: [
        {
          id: 'f-date',
          project_id: 'p1',
          name: 'Due',
          field_type: 'date',
          options: [],
          is_required: false,
          created_at: '2026-08-01T00:00:00Z',
        },
      ],
      isLoading: false,
      error: null,
    })
    mockValues.mockReturnValue({
      data: [{ field_id: 'f-date', value: '2026-12-31T00:00:00Z' }],
      isLoading: false,
      error: null,
    })

    render(wrapper(<CustomFieldsPanel issueId="i1" projectKey="TT" />))
    const input = screen.getByDisplayValue('2026-12-31')
    fireEvent.blur(input, { target: { value: '2027-01-02' } })

    expect(mockSave).toHaveBeenCalledWith({ fieldId: 'f-date', value: '2027-01-02' })
  })
})
