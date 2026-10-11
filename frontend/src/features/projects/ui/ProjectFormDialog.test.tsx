import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ProjectFormDialog } from './ProjectFormDialog'

describe('published project key semantics', () => {
  it.each(['TT', 'a-1', '7', '-', 'Ab-1234567'])('preserves valid key %s', async (key) => {
    const submit = vi.fn()
    render(<ProjectFormDialog open onOpenChange={vi.fn()} onSubmit={submit} isPending={false} />)
    const input = screen.getByLabelText(/Ключ/) as HTMLInputElement
    fireEvent.change(input, { target: { value: key } })
    fireEvent.change(screen.getByLabelText(/Название/), { target: { value: 'Project' } })
    expect(input.checkValidity()).toBe(true)
    await userEvent.click(screen.getByRole('button', { name: 'Создать проект' }))
    expect(submit).toHaveBeenCalledWith({ key, name: 'Project', description: null })
  })

  it.each(['', 'a b', 'a_b', 'abcdefghijk'])('rejects invalid key %s', async (key) => {
    const submit = vi.fn()
    render(<ProjectFormDialog open onOpenChange={vi.fn()} onSubmit={submit} isPending={false} />)
    const input = screen.getByLabelText(/Ключ/) as HTMLInputElement
    fireEvent.change(input, { target: { value: key } })
    fireEvent.change(screen.getByLabelText(/Название/), { target: { value: 'Project' } })
    expect(input.checkValidity()).toBe(false)
    await userEvent.click(screen.getByRole('button', { name: 'Создать проект' }))
    expect(submit).not.toHaveBeenCalled()
  })
})
