import { describe, it, expect, vi, beforeAll } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@pdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { AttachmentPanel } from './AttachmentPanel'

beforeAll(() => {
  i18n.changeLanguage('en')
})

const mockAttachments = vi.hoisted(() => vi.fn())
const mockUpload = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useAttachments: (...args: unknown[]) => mockAttachments(...args),
  useUploadAttachment: () => ({
    mutateAsync: mockUpload,
    isPending: false,
    isError: false,
    error: null,
  }),
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
  it('distinguishes a failed read from confirmed emptiness and retries only explicitly', () => {
    const refetch = vi.fn()
    mockAttachments.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: Object.assign(new Error('private server detail'), { status: 503 }),
      refetch,
    })
    render(wrapper(<AttachmentPanel issueId="failed-read" />))
    expect(screen.getByRole('alert')).toHaveTextContent('Could not load attachments')
    expect(screen.queryByText(/no files/i)).not.toBeInTheDocument()
    expect(refetch).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: /retry/i }))
    expect(refetch).toHaveBeenCalledOnce()
  })

  it.each([401, 403])('hides cached attachment data after access is lost with %s', (status) => {
    mockAttachments.mockReturnValue({
      data: [
        {
          id: 'hidden',
          file_name: 'sensitive.pdf',
          content_type: 'application/pdf',
          size_bytes: 64,
        },
      ],
      isLoading: false,
      error: Object.assign(new Error('private'), { status }),
      refetch: vi.fn(),
    })
    render(wrapper(<AttachmentPanel issueId="denied-read" />))
    expect(screen.queryByText('sensitive.pdf')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /sensitive.pdf/i })).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { name: /access|доступ/i })).toBeInTheDocument()
  })

  it('retains confirmed attachments with an explanation during a temporary read failure', () => {
    mockAttachments.mockReturnValue({
      data: [
        {
          id: 'retained',
          file_name: 'retained.pdf',
          content_type: 'application/pdf',
          size_bytes: 64,
        },
      ],
      isLoading: false,
      error: Object.assign(new Error('private'), { status: 503 }),
      refetch: vi.fn(),
    })
    render(wrapper(<AttachmentPanel issueId="partial-read" />))
    expect(screen.getByText('retained.pdf')).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent('Could not load attachments')
  })

  it('does not claim emptiness for an unstarted read', () => {
    mockAttachments.mockReturnValue({ data: undefined, isLoading: false, error: null })
    render(wrapper(<AttachmentPanel issueId="unstarted-read" />))
    expect(screen.queryByText(/no files/i)).not.toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'true')
  })

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

  it('keeps the upload control available after a failed request', async () => {
    mockAttachments.mockReturnValue({ data: [], isLoading: false, error: null })
    mockUpload.mockRejectedValueOnce(new Error('network failed'))
    render(wrapper(<AttachmentPanel issueId="i3" />))
    fireEvent.change(screen.getByTestId('attachment-input'), {
      target: { files: [new File(['qa'], 'qa.txt', { type: 'text/plain' })] },
    })
    expect(await screen.findByTestId('attachment-error')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /attach file/i })).toBeEnabled()
  })

  it('tracks same-named files independently until every upload settles', async () => {
    mockAttachments.mockReturnValue({ data: [], isLoading: false, error: null })
    const resolvers: Array<() => void> = []
    mockUpload.mockImplementation(() => new Promise<void>((resolve) => resolvers.push(resolve)))
    render(wrapper(<AttachmentPanel issueId="i4" />))
    fireEvent.change(screen.getByTestId('attachment-input'), {
      target: {
        files: [
          new File(['one'], 'same.txt', { type: 'text/plain' }),
          new File(['two'], 'same.txt', { type: 'text/plain' }),
        ],
      },
    })
    expect(screen.getAllByTestId('upload-progress')).toHaveLength(2)
    await waitFor(() => expect(resolvers).toHaveLength(2))
    resolvers[0]?.()
    expect(screen.getAllByTestId('upload-progress')).toHaveLength(2)
    resolvers[1]?.()
    await waitFor(() => expect(screen.queryAllByTestId('upload-progress')).toHaveLength(0))
  })

  it('exposes byte progress through the shared progressbar without changing the upload request', async () => {
    mockAttachments.mockReturnValue({ data: [], isLoading: false, error: null })
    let finish: (() => void) | undefined
    mockUpload.mockImplementationOnce(({ onProgress }) => {
      onProgress(2, 4)
      return new Promise<void>((resolve) => {
        finish = resolve
      })
    })
    render(wrapper(<AttachmentPanel issueId="progress-issue" />))
    const file = new File(['test'], 'progress.txt', { type: 'text/plain' })
    fireEvent.change(screen.getByTestId('attachment-input'), { target: { files: [file] } })
    const progressbar = await screen.findByRole('progressbar', { name: /progress.txt/ })
    expect(progressbar).toHaveAttribute('aria-valuenow', '2')
    expect(progressbar).toHaveAttribute('aria-valuemax', '4')
    expect(mockUpload).toHaveBeenLastCalledWith({ file, onProgress: expect.any(Function) })
    finish?.()
    await waitFor(() => expect(screen.queryByRole('progressbar')).not.toBeInTheDocument())
  })
})
