import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { LabelEditor, labelForeground } from './LabelEditor'

beforeAll(() => {
  i18n.changeLanguage('en')
})

const mutations = vi.hoisted(() => ({ create: vi.fn(), attach: vi.fn() }))

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
  useAttachLabel: () => ({ mutate: vi.fn(), mutateAsync: mutations.attach, isPending: false }),
  useDetachLabel: () => ({ mutate: vi.fn(), isPending: false }),
  useCreateLabel: () => ({ mutateAsync: mutations.create, isPending: false }),
}))

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

describe('LabelEditor', () => {
  it('chooses readable text for bright and dark label colors', () => {
    expect(labelForeground('#ef4444')).toBe('#000000')
    expect(labelForeground('#1e293b')).toBe('#ffffff')
  })
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

  it('retains the name and reuses a created label when attachment fails', async () => {
    mutations.create.mockResolvedValueOnce({ id: 'l3', name: 'qa' })
    mutations.attach.mockRejectedValueOnce(new Error('Attach failed')).mockResolvedValueOnce({})
    render(wrapper(<LabelEditor issueId="i1" projectKey="TT" />))
    fireEvent.click(screen.getByText(/new label/i))
    const input = screen.getByTestId('label-name-input')
    fireEvent.change(input, { target: { value: 'qa' } })
    fireEvent.click(screen.getByRole('button', { name: /^add$/i }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Attach failed')
    expect(input).toHaveValue('qa')
    fireEvent.click(screen.getByRole('button', { name: /^add$/i }))
    await waitFor(() => expect(mutations.attach).toHaveBeenCalledTimes(2))
    expect(mutations.create).toHaveBeenCalledTimes(1)
  })
})
