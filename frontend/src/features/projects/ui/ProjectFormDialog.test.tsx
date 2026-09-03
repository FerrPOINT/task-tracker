import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { ProjectFormDialog } from './ProjectFormDialog'
import type { Project } from '@/api/project'

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

describe('ProjectFormDialog', () => {
  it('renders form fields when open', () => {
    render(
      wrapper(
        <ProjectFormDialog
          open={true}
          onOpenChange={vi.fn()}
          onSubmit={vi.fn()}
          isPending={false}
        />,
      ),
    )
    expect(screen.getByPlaceholderText(/e\.g\. TT/i)).toBeInTheDocument()
    expect(screen.getByPlaceholderText(/e\.g\. Task Tracker/i)).toBeInTheDocument()
  })

  it('submits form with entered values', () => {
    const onSubmit = vi.fn()
    render(
      wrapper(
        <ProjectFormDialog
          open={true}
          onOpenChange={vi.fn()}
          onSubmit={onSubmit}
          isPending={false}
        />,
      ),
    )
    fireEvent.change(screen.getByPlaceholderText(/e\.g\. TT/i), { target: { value: 'tt' } })
    fireEvent.change(screen.getByPlaceholderText(/e\.g\. Task Tracker/i), {
      target: { value: 'Test Project' },
    })
    fireEvent.click(screen.getByRole('button', { name: /create project/i }))
    expect(onSubmit).toHaveBeenCalled()
    const arg = onSubmit.mock.calls[0]![0]
    expect(arg.key).toBe('TT')
    expect(arg.name).toBe('Test Project')
  })

  it('shows edit title when project is provided', () => {
    const project: Project = {
      id: 'p1',
      key: 'TT',
      name: 'Task Tracker',
      owner_id: 'u1',
      owner_name: 'Owner',
      todo_count: 0,
      in_progress_count: 0,
      done_count: 0,
      description: 'A tracker',
    }
    render(
      wrapper(
        <ProjectFormDialog
          open={true}
          project={project}
          onOpenChange={vi.fn()}
          onSubmit={vi.fn()}
          isPending={false}
        />,
      ),
    )
    expect(screen.getByText(/edit project/i)).toBeInTheDocument()
    expect(screen.getByDisplayValue('TT')).toBeDisabled()
  })
})
