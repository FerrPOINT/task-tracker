import { act, cleanup, render, screen, within, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { DecompositionDesignPanel } from './panel'
import { useLocation, useNavigate } from 'react-router'

afterEach(cleanup)

// Unit harness supplies navigation; the browser uses Tracker's actual router and IssueDetailPage.
function PanelHarness() {
  const location = useLocation()
  const navigate = useNavigate()
  const params = new URLSearchParams(location.search)
  const scenario = params.get('scenario') ?? 'normal'
  const board = location.pathname === '/board'
  if (board)
    return (
      <button onClick={() => navigate('/issues/DEMO-24?tab=decomposition')}>Открыть DEMO-24</button>
    )
  return (
    <>
      <button onClick={() => navigate('/board')}>← К доске проекта</button>
      {['decomposition', 'comments', 'sdlc'].map((tab) => (
        <button
          key={tab}
          role="tab"
          aria-selected={(params.get('tab') ?? 'decomposition') === tab}
          onClick={() => {
            const next = new URLSearchParams(location.search)
            next.set('tab', tab)
            navigate({ search: next.toString(), hash: '' })
          }}
        >
          {tab === 'decomposition' ? 'Декомпозиция' : tab === 'sdlc' ? 'SDLC' : 'Комментарии'}
        </button>
      ))}
      <div hidden={(params.get('tab') ?? 'decomposition') !== 'decomposition'}>
        <DecompositionDesignPanel scenario={scenario as import('./model').Scenario} />
      </div>
    </>
  )
}

function setup(scenario = 'normal', hash = '') {
  const router = createMemoryRouter([{ path: '*', element: <PanelHarness /> }], {
    initialEntries: [`/issues/DEMO-24?tab=decomposition&scenario=${scenario}${hash}`],
  })
  render(<RouterProvider router={router} />)
  return { router, user: userEvent.setup() }
}

async function edit(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: 'Редактировать' }))
  return screen.getByRole('form', { name: 'Редактирование декомпозиции' })
}

describe('memory-only decomposition preview', () => {
  it('shows a detailed example with distinct implementation boundaries and checks', async () => {
    const { user } = setup('detailed')
    expect(document.querySelectorAll('details[id^="section-"]')).toHaveLength(6)
    await user.click(screen.getByText('4. Применение настроек при отправке'))
    expect(screen.getByText(/повтор обработки не отправляет уведомление дважды/i)).toBeVisible()
    expect(screen.getByText('6. Проверка общего сценария')).toBeVisible()
  })

  it.each(['normal', 'single', 'long', 'detailed'])(
    'starts with every section collapsed for %s',
    (scenario) => {
      setup(scenario)
      const sections = document.querySelectorAll('details[id^="section-"]')
      expect(sections.length).toBeGreaterThan(0)
      sections.forEach((section) => expect(section).not.toHaveAttribute('open'))
      expect(screen.getByTestId('architecture-overview')).not.toHaveAttribute('open')
    },
  )

  it('shows an implementation plan without a second completion workflow', async () => {
    const { user } = setup()
    expect(screen.getByText('1. Хранение и API настроек')).toBeVisible()
    await user.click(screen.getByText('1. Хранение и API настроек'))
    const section = within(document.getElementById('section-storage')!)
    expect(section.getByRole('heading', { name: 'Что сделать' })).toBeVisible()
    expect(section.getByRole('heading', { name: 'Как проверить' })).toBeVisible()
    expect(screen.queryByText(/Выполнено.*из|^Выполнен$|^Не выполнен$/)).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /отмет|результат/i })).not.toBeInTheDocument()
  })

  it('reorders and renames stable sections, adds/removes a draft and saves a revision', async () => {
    const { user } = setup()
    const form = await edit(user)
    expect(screen.getByLabelText(/Архитектурное решение/)).toHaveFocus()
    await user.clear(within(form).getAllByLabelText(/Название раздела/)[0]!)
    await user.type(within(form).getAllByLabelText(/Название раздела/)[0]!, 'Хранение профиля')
    await user.click(screen.getByRole('button', { name: 'Переместить Хранение профиля вниз' }))
    expect(within(form).getAllByLabelText(/Название раздела/)[1]).toHaveValue('Хранение профиля')
    await user.click(screen.getByRole('button', { name: 'Добавить раздел' }))
    expect(within(form).getAllByLabelText(/Название раздела/)).toHaveLength(4)
    expect(
      within(screen.getByTestId('editor-new-1')).getByLabelText(/Название раздела/),
    ).toHaveFocus()
    await user.click(
      within(screen.getByTestId('editor-new-1')).getByRole('button', { name: 'Убрать раздел' }),
    )
    await user.type(screen.getByLabelText(/Причина изменения/), 'Уточнение названия и порядка')
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    expect(await screen.findByText('Сохранена редакция 3.')).toBeVisible()
    expect(screen.getByText('1. Форма настроек уведомлений')).toBeVisible()
    expect(document.getElementById('section-storage')).not.toBeNull()
    expect(screen.getByText('2. Хранение профиля')).toBeVisible()
  })

  it('cancels content edits without changing saved content or revision', async () => {
    const { user } = setup()
    await edit(user)
    await user.clear(screen.getByLabelText(/Архитектурное решение/))
    await user.type(screen.getByLabelText(/Архитектурное решение/), 'Несохранённая идея')
    await user.click(screen.getByRole('button', { name: 'Отмена' }))
    expect(screen.queryByText('Несохранённая идея')).not.toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'Редакция 2 — история изменений плана' }),
    ).toBeVisible()
    expect(screen.getByRole('button', { name: 'Редактировать' })).toHaveFocus()
  })

  it('retains a draft across tabs and guards leaving the task', async () => {
    const { user, router } = setup()
    await edit(user)
    await user.type(screen.getByLabelText(/Причина изменения/), 'Мой черновик')
    await user.click(screen.getByRole('tab', { name: 'Комментарии' }))
    expect(router.state.location.search).toContain('tab=comments')
    await user.click(screen.getByRole('tab', { name: 'Декомпозиция' }))
    expect(screen.getByLabelText(/Причина изменения/)).toHaveValue('Мой черновик')
    await user.click(screen.getByRole('button', { name: '← К доске проекта' }))
    expect(screen.getByRole('alertdialog')).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Остаться' }))
    expect(screen.getByLabelText(/Причина изменения/)).toHaveValue('Мой черновик')
    await user.click(screen.getByRole('button', { name: '← К доске проекта' }))
    await user.click(screen.getByRole('button', { name: 'Покинуть без сохранения' }))
    expect(await screen.findByRole('button', { name: 'Открыть DEMO-24' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Открыть DEMO-24' }))
    expect(screen.queryByRole('form')).not.toBeInTheDocument()
    await act(() => router.navigate(-1))
    expect(screen.getByRole('button', { name: 'Открыть DEMO-24' })).toBeVisible()
  })

  it('shows remote conflict and local draft separately; never saves before explicit reconciliation', async () => {
    const { user } = setup('conflict')
    await edit(user)
    await user.clear(screen.getByLabelText(/Архитектурное решение/))
    await user.type(screen.getByLabelText(/Архитектурное решение/), 'Локальное техническое решение')
    await user.type(screen.getByLabelText(/Причина изменения/), 'Уточнение')
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    expect(await screen.findByText('План изменился, пока вы редактировали')).toBeVisible()
    expect(screen.getByLabelText(/Архитектурное решение/)).toHaveValue(
      'Локальное техническое решение',
    )
    expect(screen.getByText(/Новая правка архитектора/)).toBeVisible()
    expect(screen.getByRole('button', { name: 'Сохранить план' })).toBeDisabled()
    await user.click(
      screen.getByRole('button', { name: 'Я сравнил изменения — продолжить правку' }),
    )
    expect(screen.getByRole('form')).toBeVisible()
    await user.type(
      screen.getByLabelText(/Архитектурное решение/),
      '. Повтор не отправляет уведомление дважды.',
    )
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    expect(await screen.findByText('Сохранена редакция 4.')).toBeVisible()
  })

  it('retains the plan draft after a save error', async () => {
    const { user } = setup('save-error')
    await edit(user)
    await user.type(screen.getByLabelText(/Причина изменения/), 'Новый порядок')
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Не удалось сохранить')
    expect(screen.getByLabelText(/Причина изменения/)).toHaveValue('Новый порядок')
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    expect(await screen.findByText('Сохранена редакция 3.')).toBeVisible()
  })

  it.each(['readonly', 'busy', 'stale'])(
    'disables writes for %s without hiding the readable plan',
    (scenario) => {
      setup(scenario)
      expect(screen.getByRole('button', { name: 'Редактировать' })).toBeDisabled()
      expect(screen.getByText('1. Хранение и API настроек')).toBeVisible()
    },
  )

  it('hides plan content after access revocation', () => {
    setup('revoked')
    expect(screen.getByText('Доступ к задаче отозван. План недоступен.')).toBeVisible()
    expect(screen.queryByText(/Выполнено.*из/)).not.toBeInTheDocument()
    expect(screen.queryByText('1. Хранение и API настроек')).not.toBeInTheDocument()
  })

  it('opens the direct section link and keeps view state through Back/Forward', async () => {
    const { user, router } = setup('normal', '#section-form')
    await waitFor(() => expect(document.getElementById('section-form')).toHaveAttribute('open'))
    expect(document.getElementById('section-storage')).not.toHaveAttribute('open')
    expect(document.getElementById('section-acceptance')).not.toHaveAttribute('open')
    await user.click(screen.getByRole('tab', { name: 'SDLC' }))
    await act(() => router.navigate(-1))
    await waitFor(() =>
      expect(screen.getByRole('tab', { name: 'Декомпозиция' })).toHaveAttribute(
        'aria-selected',
        'true',
      ),
    )
    expect(router.state.location.hash).toBe('#section-form')
    await act(() => router.navigate(1))
    await waitFor(() =>
      expect(screen.getByRole('tab', { name: 'SDLC' })).toHaveAttribute('aria-selected', 'true'),
    )
  })

  it('reads historical snapshots without write controls or duplicate section IDs', async () => {
    const { user } = setup()
    const revision = screen.getByRole('button', { name: 'Редакция 2 — история изменений плана' })
    expect(screen.queryByRole('button', { name: 'История редакций' })).not.toBeInTheDocument()
    await user.click(revision)
    const dialog = screen.getByRole('dialog')
    expect(within(dialog).queryByRole('button', { name: 'Редактировать' })).not.toBeInTheDocument()
    expect(document.querySelectorAll('#section-storage')).toHaveLength(1)
    expect(document.getElementById('snapshot-section-storage')).not.toBeNull()
    document
      .querySelectorAll('details[id^="snapshot-section-"]')
      .forEach((section) => expect(section).not.toHaveAttribute('open'))
    await user.click(within(dialog).getByRole('combobox', { name: 'Редакция' }))
    await user.click(screen.getByRole('option', { name: /Редакция 1/ }))
    await user.click(within(dialog).getByText('Архитектурное решение', { exact: true }))
    expect(
      within(dialog).getByText('Используем существующий профиль пользователя и форму настроек.'),
    ).toBeVisible()
    await user.keyboard('{Escape}')
    expect(revision).toHaveFocus()
  })

  it('creates a meaningful single-section plan in memory without network requests', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch')
    const { user } = setup('empty')
    await user.click(screen.getByRole('button', { name: 'Создать план' }))
    await user.type(screen.getByLabelText(/Архитектурное решение/), 'Изменить существующую форму')
    await user.click(screen.getByRole('button', { name: 'Добавить раздел' }))
    await user.type(screen.getByLabelText(/Название раздела/), 'Форма')
    await user.type(screen.getByLabelText(/Что сделать/), 'Добавить поле')
    await user.type(screen.getByLabelText(/Как проверить/), 'Поле сохраняется; unit tests')
    await user.type(screen.getByLabelText(/Связь с требованиями/), 'К1')
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    expect(await screen.findByText('Сохранена редакция 1.')).toBeVisible()
    expect(screen.getByText('1. Форма')).toBeVisible()
    expect(fetchSpy).not.toHaveBeenCalled()
    fetchSpy.mockRestore()
  })

  it.each(['error', 'loading'])(
    'recovers the %s read state without replacing it by an empty plan',
    async (scenario) => {
      const { user } = setup(scenario)
      expect(screen.queryByText('Декомпозиции пока нет')).not.toBeInTheDocument()
      await user.click(
        screen.getByRole('button', {
          name: scenario === 'error' ? 'Повторить' : 'Завершить демонстрационную загрузку',
        }),
      )
      expect(screen.getByText('1. Хранение и API настроек')).toBeVisible()
    },
  )
})
