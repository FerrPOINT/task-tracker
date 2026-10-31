import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { CreateTaskAction } from './CreateTaskAction'

const mocks = vi.hoisted(() => ({ navigate: vi.fn(), error: vi.fn() }))
vi.mock('@/shared/lib/project-navigation', () => ({ useProjectNavigate: () => mocks.navigate }))
vi.mock('@pdlc/ui/ui', async (load) => ({
  ...(await load<object>()),
  toast: { error: mocks.error },
}))

beforeEach(() => vi.clearAllMocks())
describe('ordinary task creation', () => {
  it('opens the real form without Fleet or an execution request', () => {
    render(<CreateTaskAction projectKey="PROJECT + Кириллица" />)
    fireEvent.click(screen.getByRole('button', { name: 'Создать' }))
    expect(mocks.navigate).toHaveBeenCalledTimes(1)
    const target = new URL(
      String(mocks.navigate.mock.lastCall?.[0]),
      'https://tracker.example.test',
    )
    expect(target.pathname).toBe('/issues/create')
    expect(target.searchParams.get('project_key')).toBe('PROJECT + Кириллица')
  })
  it('can open creation without a selected project', () => {
    render(<CreateTaskAction />)
    fireEvent.click(screen.getByRole('button', { name: 'Создать' }))
    expect(mocks.navigate).toHaveBeenCalledWith('/issues/create?source=navigation')
  })
  it('does not discard a failed project context', () => {
    render(<CreateTaskAction projectKey="CORE" contextError="Проект недоступен" />)
    fireEvent.click(screen.getByRole('button', { name: 'Создать' }))
    expect(mocks.navigate).not.toHaveBeenCalled()
    expect(mocks.error).toHaveBeenCalledWith('Проект недоступен')
  })
})
