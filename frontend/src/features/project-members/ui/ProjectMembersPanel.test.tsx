import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@/shared/lib/theme'
import i18n from '@/shared/i18n/config'
import { ProjectMembersPanel } from './ProjectMembersPanel'

beforeAll(() => {
  i18n.changeLanguage('en')
})

vi.mock('@/shared/api/hooks', () => ({
  useProjectMembers: () => ({
    data: {
      members: [
        { user_id: 'u1', role: 'admin', project_id: 'p1', joined_at: '2026-08-01T00:00:00Z' },
        { user_id: 'u2', role: 'member', project_id: 'p1', joined_at: '2026-08-02T00:00:00Z' },
      ],
    },
    isLoading: false,
    error: null,
  }),
  useUsers: () => ({
    data: [
      { id: 'u1', username: 'alice', display_name: 'Alice', email: 'alice@test.com' },
      { id: 'u2', username: 'bob', display_name: 'Bob', email: 'bob@test.com' },
      { id: 'u3', username: 'charlie', display_name: 'Charlie', email: 'charlie@test.com' },
    ],
    isLoading: false,
    error: null,
  }),
  useAddProjectMember: () => ({ mutate: vi.fn(), isPending: false }),
  useRemoveProjectMember: () => ({ mutate: vi.fn(), isPending: false }),
}))

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

describe('ProjectMembersPanel', () => {
  it('renders member list in dialog', () => {
    render(wrapper(<ProjectMembersPanel projectId="p1" />))
    fireEvent.click(screen.getByText(/members/i))
    expect(screen.getByText('Alice')).toBeInTheDocument()
    expect(screen.getByText('Bob')).toBeInTheDocument()
  })

  it('shows remove button for members', () => {
    render(wrapper(<ProjectMembersPanel projectId="p1" />))
    fireEvent.click(screen.getByText(/members/i))
    expect(screen.getByLabelText(/remove.*alice/i)).toBeInTheDocument()
  })
})