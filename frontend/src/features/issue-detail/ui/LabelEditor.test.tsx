import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { LabelEditor } from './LabelEditor'

beforeAll(() => {
  i18n.changeLanguage('en')
})

vi.mock('@/shared/api/hooks', () => ({
  useProjectLabels: () => ({
    data: [
      { id: 'l1', name: 'bug', color: '#ef4444', project_id: 'p1' },
      { id: 'l2', name: 'feature', color: '#22c55e', project_id: 'p1' },
    ],
    isLoading: false,
    error: null,
  }),
  useIssueLabels: () => ({
    data: [{ id: 'l1', name: 'bug', color: '#ef4444', project_id: 'p1' }],
    isLoading: false,
    error: null,
  }),
  useAttachLabel: () => ({ mutate: vi.fn(), isPending: false }),
  useDetachLabel: () => ({ mutate: vi.fn(), isPending: false }),
  useCreateLabel: () => ({ mutateAsync: vi.fn(), isPending: false }),
}))

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

describe('LabelEditor', () => {
  it('renders issue labels', () => {
    render(wrapper(<LabelEditor issueId="i1" projectKey="TT" />))
    expect(screen.getByTestId('label-editor')).toBeInTheDocument()
    expect(screen.getByText('bug')).toBeInTheDocument()
  })

  it('shows create label form on button click', () => {
    render(wrapper(<LabelEditor issueId="i1" projectKey="TT" />))
    fireEvent.click(screen.getByText(/new label/i))
    expect(screen.getByTestId('label-name-input')).toBeInTheDocument()
  })
})
