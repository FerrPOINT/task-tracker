import { beforeEach, afterEach, expect, it, vi } from 'vitest'
import { cleanup, render, screen, within, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { I18nextProvider } from 'react-i18next'
import i18n from '@/shared/i18n/test-config'
import { pdlcLocales } from '@pdlc/ui/i18n'
import { CommentsPanel } from './CommentList'

const mocks = vi.hoisted(() => ({ remove: vi.fn(), pending: false, writing: false }))
vi.mock('../model/use-comments', () => ({
  useComments: () => ({
    data: [
      {
        id: 'comment-42',
        issueId: 'issue-1',
        authorId: 'user-1',
        authorName: 'Demo',
        body: 'Keep this comment',
        createdAt: '2026-10-01T10:00:00Z',
        updatedAt: '2026-10-01T10:00:00Z',
      },
    ],
    isLoading: false,
  }),
  useCreateComment: () => ({ mutateAsync: vi.fn(), isPending: mocks.writing }),
  useUpdateComment: () => ({ mutateAsync: vi.fn() }),
  useDeleteComment: () => ({ mutate: mocks.remove, isPending: mocks.pending }),
}))
beforeEach(() => {
  i18n.addResource('ru', 'translation', 'common.confirm', pdlcLocales.ru.common.confirm)
  mocks.remove.mockReset()
  mocks.pending = false
  mocks.writing = false
})
afterEach(cleanup)
const panel = () => (
  <I18nextProvider i18n={i18n}>
    <CommentsPanel issueId="issue-1" currentUserId="user-1" />
  </I18nextProvider>
)

it('cancels without a write, returns focus and retains a failed deletion for explicit retry', async () => {
  const user = userEvent.setup()
  render(panel())
  const opener = screen.getByRole('button', { name: 'Удалить' })
  await user.click(opener)
  await screen.findByRole('alertdialog')
  expect(mocks.remove).not.toHaveBeenCalled()
  await user.keyboard('{Escape}')
  expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
  expect(opener).toHaveFocus()
  await user.click(opener)
  mocks.remove.mockImplementationOnce((_id, handlers) =>
    handlers.onError(new Error('Delete failed')),
  )
  await user.click(
    within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Подтвердить' }),
  )
  expect(await screen.findByRole('alert')).toHaveTextContent('Delete failed')
  expect(screen.getByText('Keep this comment')).toBeInTheDocument()
  expect(mocks.remove).toHaveBeenCalledExactlyOnceWith('comment-42', expect.any(Object))
  mocks.remove.mockImplementationOnce((_id, handlers) => handlers.onSuccess())
  await user.click(
    within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Подтвердить' }),
  )
  await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
  expect(mocks.remove).toHaveBeenLastCalledWith('comment-42', expect.any(Object))
  expect(mocks.remove).toHaveBeenCalledTimes(2)
})

it('keeps the pending confirmation open and prevents cancellation or duplicate writes', async () => {
  const user = userEvent.setup()
  const view = render(panel())
  await user.click(screen.getByRole('button', { name: 'Удалить' }))
  mocks.pending = true
  view.rerender(panel())
  const dialog = screen.getByRole('alertdialog')
  expect(within(dialog).getByRole('button', { name: 'Подтвердить' })).toBeDisabled()
  expect(within(dialog).getByRole('button', { name: 'Отмена' })).toBeDisabled()
  await user.keyboard('{Escape}')
  expect(dialog).toBeInTheDocument()
  expect(mocks.remove).not.toHaveBeenCalled()
  mocks.pending = false
  view.rerender(panel())
  await user.click(within(dialog).getByRole('button', { name: 'Отмена' }))
  expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
})

it('keeps the active composer while its write is pending instead of switching to an existing comment', async () => {
  const user = userEvent.setup()
  const view = render(panel())
  await user.click(screen.getByRole('button', { name: 'Добавить комментарий' }))
  const editor = screen.getByRole('textbox')
  await user.type(editor, 'Pending draft')
  mocks.writing = true
  view.rerender(panel())
  expect(screen.getByRole('button', { name: 'Изменить' })).toBeDisabled()
  await user.click(screen.getByRole('button', { name: 'Изменить' }))
  expect(screen.getByRole('textbox')).toBe(editor)
  expect(editor).toHaveValue('Pending draft')
  mocks.writing = false
  view.rerender(panel())
  await user.click(screen.getByRole('button', { name: 'Изменить' }))
  expect(screen.getByRole('textbox')).toHaveValue('Keep this comment')
})
