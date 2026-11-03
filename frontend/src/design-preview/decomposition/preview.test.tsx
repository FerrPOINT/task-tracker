import { act, cleanup, render, screen, within, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { DecompositionPreview } from './preview'
import { fixture, revise } from './model'

afterEach(cleanup)

function setup(scenario = 'normal', hash = '') {
  const router = createMemoryRouter([{ path: '*', element: <DecompositionPreview /> }], {
    initialEntries: [`/decomposition-preview.html?tab=decomposition&scenario=${scenario}${hash}`],
  })
  render(<RouterProvider router={router} />)
  return { router, user: userEvent.setup() }
}

async function edit(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: 'Редактировать' }))
  return screen.getByRole('form', { name: 'Редактирование декомпозиции' })
}

describe('memory-only decomposition preview', () => {
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
    expect(screen.getByText('Выполнено 1 из 3')).toBeVisible()
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
    expect(screen.getByText('Редакция 2 · Архитектор')).toBeVisible()
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

  it('retains the failed result input and changes progress only after a successful retry', async () => {
    const { user } = setup('save-error')
    await user.click(screen.getByText('2. Форма настроек уведомлений'))
    const section = document.getElementById('section-form')!
    await user.click(within(section).getByRole('button', { name: 'Отметить выполненным' }))
    const dialog = screen.getByRole('dialog')
    await user.type(within(dialog).getByLabelText(/Что выполнено/), 'Форма сохраняет выбор')
    await user.type(
      within(dialog).getByLabelText(/Изменения и проверки/),
      'example-commit; UI tests 8/8',
    )
    await user.click(within(dialog).getByRole('button', { name: 'Сохранить результат' }))
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('Не удалось сохранить')
    expect(within(dialog).getByLabelText(/Что выполнено/)).toHaveValue('Форма сохраняет выбор')
    expect(screen.getByText('Выполнено 1 из 3')).toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: 'Сохранить результат' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(screen.getByText('Выполнено 2 из 3')).toBeVisible()
    expect(screen.getByText('Редакция 2 · Архитектор')).toBeVisible()
    await user.click(within(section).getByRole('button', { name: 'Снять отметку выполнения' }))
    await user.type(screen.getByLabelText(/Причина снятия отметки/), 'Найден дефект повтора')
    await user.click(screen.getByRole('button', { name: 'Снять отметку' }))
    expect(await screen.findByText('Отметка снята: Найден дефект повтора')).toBeVisible()
    expect(screen.getByText('Выполнено 1 из 3')).toBeVisible()
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
      expect(screen.getByRole('button', { name: 'Снять отметку выполнения' })).toBeDisabled()
      expect(screen.getByText('Выполнено 1 из 3')).toBeVisible()
    },
  )

  it('hides plan content and progress after access revocation', () => {
    setup('revoked')
    expect(screen.getByText('Доступ к задаче отозван. План и прогресс недоступны.')).toBeVisible()
    expect(screen.queryByText(/Выполнено.*из/)).not.toBeInTheDocument()
    expect(screen.queryByText('1. Хранение и API настроек')).not.toBeInTheDocument()
  })

  it('opens the direct section link and keeps view state through Back/Forward', async () => {
    const { user, router } = setup('normal', '#section-form')
    await waitFor(() => expect(document.getElementById('section-form')).toHaveAttribute('open'))
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
    await user.click(screen.getByRole('button', { name: 'История редакций' }))
    const dialog = screen.getByRole('dialog')
    expect(
      within(dialog).queryByRole('button', { name: 'Снять отметку выполнения' }),
    ).not.toBeInTheDocument()
    expect(document.querySelectorAll('#section-storage')).toHaveLength(1)
    expect(document.getElementById('snapshot-section-storage')).not.toBeNull()
    await user.keyboard('{Escape}')
    expect(screen.getByRole('button', { name: 'История редакций' })).toHaveFocus()
  })

  it('creates a meaningful single-section plan in memory without network requests', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch')
    const { user } = setup('empty')
    await user.click(screen.getByRole('button', { name: 'Создать план' }))
    await user.type(screen.getByLabelText(/Архитектурное решение/), 'Изменить существующую форму')
    await user.click(screen.getByRole('button', { name: 'Добавить раздел' }))
    await user.type(screen.getByLabelText(/Название раздела/), 'Форма')
    await user.type(screen.getByLabelText(/Описание и результат/), 'Добавить поле')
    await user.type(screen.getByLabelText(/Критерии готовности/), 'Поле сохраняется; unit tests')
    await user.type(screen.getByLabelText(/Связь с требованиями/), 'К1')
    await user.click(screen.getByRole('button', { name: 'Сохранить план' }))
    expect(await screen.findByText('Сохранена редакция 1.')).toBeVisible()
    expect(screen.getByText('Выполнено 0 из 1')).toBeVisible()
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
      expect(screen.getByText('Выполнено 1 из 3')).toBeVisible()
    },
  )
})

it('invalidates changed work while preserving renamed unchanged work', () => {
  const before = fixture('normal')!
  const renamed = structuredClone(before)
  renamed.sections[0]!.title = 'Новое название'
  expect(revise(before, renamed, 'Название').sections[0]!.result).not.toBeNull()
  renamed.sections[0]!.criteria = 'Другое поведение'
  expect(revise(before, renamed, 'Критерий').sections[0]!.result).toBeNull()
  const architectureChanged = structuredClone(before)
  architectureChanged.overview = 'Изменён общий способ хранения'
  expect(revise(before, architectureChanged, 'Архитектура').sections[0]!.result).toBeNull()
})
