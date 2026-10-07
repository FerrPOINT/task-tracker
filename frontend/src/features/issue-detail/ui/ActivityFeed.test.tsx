import { describe, it, expect, beforeAll } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { ActivityFeed } from './ActivityFeed'
import type { Comment } from '@/entities/comment/model'
import type { Worklog } from '@/entities/worklog/model'

beforeAll(() => {
  i18n.changeLanguage('en')
})

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

const comment: Comment = {
  id: 'c1',
  issueId: 'i1',
  authorId: 'u1',
  authorName: 'Alice',
  body: 'Looks good',
  createdAt: '2026-08-01T10:00:00Z',
  updatedAt: '2026-08-01T10:00:00Z',
}

const worklog: Worklog = {
  id: 'w1',
  issueId: 'i1',
  userId: 'u2',
  userDisplayName: 'Bob',
  timeSpentSeconds: 3600,
  startedAt: '2026-08-01T12:00:00Z',
  comment: 'Working on it',
  createdAt: '2026-08-01T12:00:00Z',
  updatedAt: '2026-08-01T12:00:00Z',
}

describe('ActivityFeed', () => {
  it('renders safe Markdown comments in activity', () => {
    const markdownComment = {
      ...comment,
      body: '# Итоги\n\n**Результат**\n\nПервая строка\nВторая строка\n\n- Первый\n- Второй\n\n```text\nfirst line\n  indented line\n```\n\n<script>alert(1)</script>\n\n[Опасно](javascript:alert%281%29)',
    }
    const { container } = render(
      wrapper(<ActivityFeed comments={[markdownComment]} worklogs={[]} />),
    )
    expect(screen.getByRole('heading', { name: 'Итоги', level: 1 })).toBeVisible()
    expect(screen.getByText('Результат').tagName).toBe('STRONG')
    expect(screen.getByText('Первая строка Вторая строка').textContent).toBe(
      'Первая строка\nВторая строка',
    )
    expect(screen.getAllByRole('listitem')).toHaveLength(2)
    expect(container.querySelector('pre code')?.textContent).toBe('first line\n  indented line\n')
    expect(container.querySelector('script')).toBeNull()
    expect(container.querySelector('[href^="javascript:"]')).toBeNull()
  })

  it('renders comment and worklog activity items', () => {
    render(wrapper(<ActivityFeed comments={[comment]} worklogs={[worklog]} />))
    expect(screen.getByText('Alice')).toBeInTheDocument()
    expect(screen.getByText('Looks good')).toBeInTheDocument()
    expect(screen.getByText('Bob')).toBeInTheDocument()
    expect(screen.getByText('Working on it')).toBeInTheDocument()
  })

  it('renders empty state when no activity', () => {
    render(wrapper(<ActivityFeed comments={[]} worklogs={[]} />))
    expect(screen.getByText(/no activity yet/i)).toBeInTheDocument()
  })
})
