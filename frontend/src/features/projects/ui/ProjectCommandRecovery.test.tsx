import { afterEach, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, renderHook, screen } from '@testing-library/react'
import { Button, Input } from '@pdlc/ui/ui'
import { useSessionCommand } from '@pdlc/ui/lib'
import { ProjectCommandRecovery } from './ProjectCommandRecovery'

afterEach(() => {
  cleanup()
  sessionStorage.clear()
  vi.restoreAllMocks()
})

it('preserves the healthy native form, focus and input on an ordinary owner rerender', () => {
  const view = (label: string) => (
    <ProjectCommandRecovery>
      <form aria-label="Проект">
        <Input name="name" aria-label={label} defaultValue="Исходное" />
      </form>
    </ProjectCommandRecovery>
  )
  const { container, rerender } = render(view('Название'))
  const form = screen.getByRole('form') as HTMLFormElement
  const input = screen.getByRole('textbox')
  fireEvent.change(input, { target: { value: 'Сохранённый ввод' } })
  input.focus()
  rerender(view('Название проекта'))
  expect(container.firstElementChild).toBe(form)
  expect(screen.getByRole('textbox')).toBe(input)
  expect(document.activeElement).toBe(input)
  expect([...new FormData(form)]).toEqual([['name', 'Сохранённый ввод']])
})

it.each([
  { operation_id: 'original-a', name: 'Проект A', spaces: ['a', 'b'] },
  { operation_id: 'original-b', name: 'Проект с длинным названием', spaces: ['b'] },
])(
  'keeps the original command through render failure and requires owner identity reset: $operation_id',
  (command) => {
    const context = 'tracker:create-project-v2:' + JSON.stringify(['issuer', command.operation_id])
    const key = 'pdlc:command:' + context
    const initial = renderHook(() => useSessionCommand<typeof command>(context))
    act(() => {
      initial.result.current.capture(command)
    })
    initial.unmount()
    const bytes = sessionStorage.getItem(key)
    const writes = vi.spyOn(Storage.prototype, 'setItem')
    const removals = vi.spyOn(Storage.prototype, 'removeItem')
    const send = vi.fn()
    function Form({ broken }: { broken: boolean }) {
      const journal = useSessionCommand<typeof command>(context)
      if (broken) throw new TypeError('Synthetic project form render failure')
      return (
        <form aria-label="Проект" onSubmit={(event) => event.preventDefault()}>
          <Input aria-label="Название" defaultValue={journal.command?.name} />
          <Button
            type="button"
            onClick={() => send(journal.capture({ ...command, name: 'Новый ввод' }))}
          >
            Проверить исходную команду
          </Button>
        </form>
      )
    }
    const view = (identity: string, broken: boolean) => (
      <ProjectCommandRecovery key={identity}>
        <Form broken={broken} />
      </ProjectCommandRecovery>
    )
    const caught = vi.fn()
    const { rerender } = render(view('first', false), { onCaughtError: caught })
    rerender(view('first', true))
    expect(
      screen.getByText(
        'Не удалось открыть форму проекта. Сохранённый журнал не удалён; требуется проверка ошибки.',
      ),
    ).toBeTruthy()
    expect(screen.queryByRole('textbox')).toBeNull()
    expect(caught).toHaveBeenCalledTimes(1)
    expect(sessionStorage.getItem(key)).toBe(bytes)
    expect(send).not.toHaveBeenCalled()
    rerender(view('first', false))
    expect(screen.queryByRole('textbox')).toBeNull()
    expect(caught).toHaveBeenCalledTimes(1)
    expect(sessionStorage.getItem(key)).toBe(bytes)
    rerender(view('next', false))
    expect((screen.getByRole('textbox') as HTMLInputElement).value).toBe(command.name)
    expect(send).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Проверить исходную команду' }))
    expect(send).toHaveBeenCalledExactlyOnceWith(command)
    expect(sessionStorage.getItem(key)).toBe(bytes)
    expect(writes).not.toHaveBeenCalled()
    expect(removals).not.toHaveBeenCalled()
  },
)
