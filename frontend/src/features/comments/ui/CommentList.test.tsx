import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { I18nextProvider } from 'react-i18next'
import i18n from '@/shared/i18n/test-config'
import { CommentList, CommentForm, CommentItem } from './CommentList'
import type { Comment } from '@/entities/comment/model'

function Wrapper({ children }: { children: React.ReactNode }) {
  return <I18nextProvider i18n={i18n}>{children}</I18nextProvider>
}

const sampleComment: Comment = {
  id: 'c1',
  issueId: 'i1',
  authorId: 'u1',
  authorName: 'Demo User',
  body: 'Test comment',
  createdAt: '2026-07-30T10:00:00Z',
  updatedAt: '2026-07-30T10:00:00Z',
}

describe('CommentList', () => {
  it('renders empty state', () => {
    render(
      <Wrapper>
        <CommentList comments={[]} onEdit={() => {}} onDelete={() => {}} />
      </Wrapper>,
    )
    expect(screen.getByText(/пока нет комментариев/i)).toBeInTheDocument()
  })

  it('renders comment item', () => {
    render(
      <Wrapper>
        <CommentList
          comments={[sampleComment]}
          currentUserId="u1"
          onEdit={() => {}}
          onDelete={() => {}}
        />
      </Wrapper>,
    )
    expect(screen.getByText('Test comment')).toBeInTheDocument()
    expect(screen.getByText('Demo User')).toBeInTheDocument()
    expect(screen.getByText(/изменить/i)).toBeInTheDocument()
  })
})

describe('CommentItem', () => {
  it('does not show actions for non-author', () => {
    render(
      <Wrapper>
        <CommentItem comment={sampleComment} onEdit={() => {}} onDelete={() => {}} />
      </Wrapper>,
    )
    expect(screen.queryByText(/изменить/i)).not.toBeInTheDocument()
  })
})

describe('CommentForm', () => {
  it('submits non-empty body', async () => {
    const onSubmit = vi.fn()
    render(
      <Wrapper>
        <CommentForm onSubmit={onSubmit} submitLabel="Add" />
      </Wrapper>,
    )
    fireEvent.change(screen.getByPlaceholderText(/напишите/i), {
      target: { value: 'New comment' },
    })
    fireEvent.click(screen.getByText(/add/i))
    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith({ body: 'New comment' }))
  })
})
