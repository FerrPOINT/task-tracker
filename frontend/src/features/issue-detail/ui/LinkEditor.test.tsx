import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { LinkEditor } from './LinkEditor'

beforeAll(() => {
  i18n.changeLanguage('en')
})

vi.mock('@/shared/api/hooks', () => ({
  useIssueLinks: () => ({
    data: [
      {
        id: 'lk1',
        link_type: 'blocks',
        source_id: 'i1',
        source_key: 'TT-1',
        target_id: 'i2',
        target_key: 'TT-2',
      },
    ],
    isLoading: false,
    error: null,
  }),
  useCreateIssueLink: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useDeleteIssueLink: () => ({ mutate: vi.fn(), isPending: false }),
}))

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

describe('LinkEditor', () => {
  it('renders existing issue links', () => {
    render(wrapper(<LinkEditor issueId="i1" currentKey="TT-1" />))
    expect(screen.getByTestId('link-editor')).toBeInTheDocument()
    expect(screen.getByText('TT-2')).toBeInTheDocument()
  })

  it('links to the related issue UUID, not its display key', () => {
    render(wrapper(<LinkEditor issueId="i1" currentKey="TT-1" />))
    expect(screen.getByRole('link', { name: 'TT-2' })).toHaveAttribute('href', '/issues/i2')
  })

  it('shows add link form on button click', () => {
    render(wrapper(<LinkEditor issueId="i1" currentKey="TT-1" />))
    fireEvent.click(screen.getByText(/add link/i))
    expect(screen.getByTestId('link-target-input')).toBeInTheDocument()
    expect(screen.getByTestId('link-submit')).toBeInTheDocument()
  })
})
