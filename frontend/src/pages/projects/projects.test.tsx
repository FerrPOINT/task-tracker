import { afterEach, beforeEach, describe, it, expect, vi } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'

import { ProjectsPage } from './'
import { ThemeProvider } from '@sdlc/ui/lib'
import i18n from '@/shared/i18n/config'

const useProjects = vi.hoisted(() => vi.fn())
const useCreateProject = vi.hoisted(() => vi.fn())
const useUpdateProject = vi.hoisted(() => vi.fn())
const useDeleteProject = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useProjects,
  useCreateProject,
  useUpdateProject,
  useDeleteProject,
}))

const projects = [
  {
    id: 'p1',
    key: 'TT',
    name: 'Task Tracker',
    owner_id: 'u1',
    todo_count: 2,
    in_progress_count: 3,
    done_count: 1,
  },
  {
    id: 'p2',
    key: 'DOC',
    name: 'Knowledge Base',
    owner_id: 'u2',
    todo_count: 1,
    in_progress_count: 0,
    done_count: 4,
  },
]

function renderPage() {
  return render(
    <ThemeProvider>
      <MemoryRouter>
        <ProjectsPage />
      </MemoryRouter>
    </ThemeProvider>,
  )
}

beforeEach(() => {
  useProjects.mockReturnValue({ data: projects, isLoading: false, error: null, refetch: vi.fn() })
  useCreateProject.mockReturnValue({ mutate: vi.fn(), isPending: false, error: null })
  useUpdateProject.mockReturnValue({ mutate: vi.fn(), isPending: false, error: null })
  useDeleteProject.mockReturnValue({
    mutate: vi.fn(),
    reset: vi.fn(),
    isPending: false,
    error: null,
  })
})

afterEach(async () => {
  await act(async () => {
    await i18n.changeLanguage('ru')
  })
  vi.clearAllMocks()
})

describe('ProjectsPage', () => {
  it('renders compact project rows with distinct links and actions', () => {
    renderPage()
    expect(screen.getByRole('list').querySelectorAll('li')).toHaveLength(2)
    expect(screen.getByRole('link', { name: /Task Tracker/i })).toHaveAttribute(
      'href',
      '/projects/TT/board',
    )
    expect(
      screen.getByRole('button', { name: 'Действия с проектом Task Tracker' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'Действия с проектом Knowledge Base' }),
    ).toBeInTheDocument()
    expect(screen.getAllByText('К выполнению')).toHaveLength(2)
  })

  it('shows a localized no-match state after filtering', async () => {
    await act(async () => {
      await i18n.changeLanguage('en')
    })
    renderPage()
    fireEvent.change(screen.getByRole('textbox', { name: /search projects/i }), {
      target: { value: 'not found' },
    })
    expect(screen.getByText('No projects match your search.')).toBeInTheDocument()
    expect(screen.queryByRole('list')).not.toBeInTheDocument()
  })

  it('offers retry when projects fail to load', () => {
    const refetch = vi.fn()
    useProjects.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('offline'),
      refetch,
    })
    renderPage()
    expect(screen.getByText('offline')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /повторить|retry/i }))
    expect(refetch).toHaveBeenCalledOnce()
  })

  it('keeps the confirmation open while deletion runs or fails', async () => {
    const remove = {
      mutate: vi.fn(),
      reset: vi.fn(),
      isPending: false,
      error: null as Error | null,
    }
    useDeleteProject.mockReturnValue(remove)
    const view = renderPage()

    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'Действия с проектом Task Tracker' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Удалить' }))
    expect(screen.getByText(/Проект «Task Tracker»/)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Удалить' }))
    expect(remove.mutate).toHaveBeenCalledWith(
      'TT',
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    )
    expect(screen.getByRole('alertdialog')).toBeInTheDocument()

    remove.isPending = true
    view.rerender(
      <ThemeProvider>
        <MemoryRouter>
          <ProjectsPage />
        </MemoryRouter>
      </ThemeProvider>,
    )
    expect(screen.getByRole('button', { name: 'Отмена' })).toBeDisabled()
    expect(screen.getByRole('button', { name: /загрузка/i })).toBeDisabled()

    remove.isPending = false
    remove.error = new Error('Удаление не удалось')
    view.rerender(
      <ThemeProvider>
        <MemoryRouter>
          <ProjectsPage />
        </MemoryRouter>
      </ThemeProvider>,
    )
    expect(screen.getByRole('alert')).toHaveTextContent('Не удалось удалить проект')
    expect(screen.getByRole('alertdialog')).toBeInTheDocument()
  })
})
