import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { AttachmentPanel } from './AttachmentPanel'

beforeAll(() => {
  i18n.changeLanguage('en')
})

const mockAttachments = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useAttachments: (...args: unknown[]) => mockAttachments(...args),
  useUploadAttachment: () => ({ mutate: vi.fn(), isPending: false, isError: false, error: null }),
  useDeleteAttachment: () => ({ mutate: vi.fn(), isPending: false }),
}))

vi.mock('@/api/attachment', () => ({
  downloadAttachment: vi.fn(),
}))

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

describe('AttachmentPanel', () => {
  it('renders attachment list and upload button', () => {
    mockAttachments.mockReturnValue({
      data: [
        {
          id: 'a1',
          issue_id: 'i1',
          author_id: 'u1',
          file_name: 'report.pdf',
          content_type: 'application/pdf',
          size_bytes: 2048,
          created_at: '2026-08-01T10:00:00Z',
        },
      ],
      isLoading: false,
      error: null,
    })
    render(wrapper(<AttachmentPanel issueId="i1" />))
    expect(screen.getByText('report.pdf')).toBeInTheDocument()
    expect(screen.getByTestId('attachment-panel')).toBeInTheDocument()
  })

  it('shows empty state when no attachments', () => {
    mockAttachments.mockReturnValue({
      data: [],
      isLoading: false,
      error: null,
    })
    render(wrapper(<AttachmentPanel issueId="i2" />))
    expect(screen.getByText(/no files/i)).toBeInTheDocument()
  })
})
