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
  it('preserves soft line breaks in paragraphs and list items without changing code blocks', async () => {
    const body =
      'Первая строка\nВторая строка\n\n- Первый пункт\n  Продолжение пункта\n\n```text\nfirst line\n  indented line\n```'
    const { container } = render(
      <Wrapper>
        <CommentItem comment={{ ...sampleComment, body }} onEdit={() => {}} onDelete={() => {}} />
      </Wrapper>,
    )
    const paragraph = screen.getByText('Первая строка Вторая строка')
    expect(paragraph.textContent).toBe('Первая строка\nВторая строка')
    expect(paragraph.parentElement).toHaveClass('[&_p]:whitespace-pre-line')
    const listItem = screen.getByRole('listitem')
    expect(listItem.textContent).toBe('Первый пункт\nПродолжение пункта')
    expect(paragraph.parentElement).toHaveClass('[&_li]:whitespace-pre-line')
    expect(container.querySelector('pre code')?.textContent).toBe('first line\n  indented line\n')
    const { compile } = await import('tailwindcss')
    const compiler = await compile('@tailwind utilities;')
    const style = document.createElement('style')
    style.textContent = compiler.build(Array.from(paragraph.parentElement!.classList))
    document.head.append(style)
    try {
      expect(paragraph).toHaveStyle({ whiteSpace: 'pre-line' })
      expect(listItem).toHaveStyle({ whiteSpace: 'pre-line' })
      expect(container.querySelector('pre')).toHaveStyle({ whiteSpace: 'pre' })
    } finally {
      style.remove()
    }
  })

  it('renders Markdown, not raw syntax, without executing HTML or unsafe links', () => {
    const body =
      '# Итоги\n\n**Результат** и *курсив*\n\n- Первый\n- Второй\n\n[Evidence](https://example.test/report)\n\n```text\ncode <tag>\n```\n\n<script>alert(1)</script>\n\n[Опасно](javascript:alert%281%29)'
    const { container } = render(
      <Wrapper>
        <CommentItem comment={{ ...sampleComment, body }} onEdit={() => {}} onDelete={() => {}} />
      </Wrapper>,
    )
    expect(screen.getByRole('heading', { name: 'Итоги' })).toBeVisible()
    expect(screen.getByText('Результат').tagName).toBe('STRONG')
    expect(screen.getByText('курсив').tagName).toBe('EM')
    expect(screen.getAllByRole('listitem')).toHaveLength(2)
    expect(screen.getByRole('link', { name: 'Evidence' })).toHaveAttribute(
      'href',
      'https://example.test/report',
    )
    expect(container.querySelector('pre code')).toHaveTextContent('code <tag>')
    expect(container.querySelector('script')).toBeNull()
    expect(container.querySelector('[href^="javascript:"]')).toBeNull()
  })

  it('passes the unchanged Markdown source to edit', () => {
    const comment = { ...sampleComment, body: '**Исходник**\n\n- пункт' }
    const onEdit = vi.fn()
    render(
      <Wrapper>
        <CommentItem comment={comment} currentUserId="u1" onEdit={onEdit} onDelete={() => {}} />
      </Wrapper>,
    )
    fireEvent.click(screen.getByText(/изменить/i))
    expect(onEdit).toHaveBeenCalledWith(comment)
  })

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
  it('does not submit the same comment twice while the mutation is pending', async () => {
    let resolveSubmit: (() => void) | undefined
    const onSubmit = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveSubmit = resolve
        }),
    )
    render(
      <Wrapper>
        <CommentForm onSubmit={onSubmit} submitLabel="Add" />
      </Wrapper>,
    )
    fireEvent.change(screen.getByPlaceholderText(/напишите/i), {
      target: { value: 'Only once' },
    })
    const submit = screen.getByText(/add/i)
    fireEvent.click(submit)
    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1))
    // Do not clear the text while the request is still in flight.
    expect(screen.getByPlaceholderText(/напишите/i)).toHaveValue('Only once')
    expect(submit).toBeDisabled()
    resolveSubmit?.()
    await waitFor(() => expect(screen.getByPlaceholderText(/напишите/i)).toHaveValue(''))
  })

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

  it('retains the draft and shows an error when saving fails', async () => {
    const onSubmit = vi.fn().mockRejectedValue(new Error('Network failed'))
    render(
      <Wrapper>
        <CommentForm onSubmit={onSubmit} submitLabel="Add" />
      </Wrapper>,
    )
    const textarea = screen.getByPlaceholderText(/напишите/i)
    fireEvent.change(textarea, { target: { value: 'Keep this draft' } })
    fireEvent.click(screen.getByText(/add/i))
    expect(await screen.findByRole('alert')).toHaveTextContent('Network failed')
    expect(textarea).toHaveValue('Keep this draft')
    expect(screen.getByText(/add/i)).toBeEnabled()
  })
})
