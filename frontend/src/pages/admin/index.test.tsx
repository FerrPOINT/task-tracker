import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'
import { AdminPage } from './index'

const useAdminUsers = vi.hoisted(() => vi.fn())
const useAdminSettings = vi.hoisted(() => vi.fn())
const useAdminAuditLog = vi.hoisted(() => vi.fn())
const useCreateAdminUser = vi.hoisted(() => vi.fn())
const useUpdateAdminUserStatus = vi.hoisted(() => vi.fn())
const useUpdateAdminSetting = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useAdminUsers,
  useAdminSettings,
  useAdminAuditLog,
  useCreateAdminUser,
  useUpdateAdminUserStatus,
  useUpdateAdminSetting,
}))

const mutate = vi.fn()

function renderPage() {
  return render(
    <MemoryRouter>
      <AdminPage />
    </MemoryRouter>,
  )
}

describe('AdminPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useAdminUsers.mockReturnValue({
      data: [
        {
          id: 'user-1',
          email: 'admin@example.test',
          username: 'admin',
          display_name: 'Administrator',
          is_system_admin: true,
          is_active: true,
        },
      ],
      isLoading: false,
      error: null,
    })
    useAdminSettings.mockReturnValue({
      data: [{ key: 'instance.name', value: 'Task Tracker', updated_at: '2026-08-25T10:00:00Z' }],
      isLoading: false,
      error: null,
    })
    useAdminAuditLog.mockReturnValue({
      data: [
        {
          id: 'audit-1',
          action: 'user.created',
          entity_type: 'user',
          entity_id: 'user-1',
          actor_id: 'admin-1',
          metadata: { email: 'new@example.test' },
          created_at: '2026-08-25T10:00:00Z',
        },
      ],
      isLoading: false,
      error: null,
    })
    useCreateAdminUser.mockReturnValue({ mutate, isPending: false, error: null })
    useUpdateAdminUserStatus.mockReturnValue({ mutate, isPending: false, error: null })
    useUpdateAdminSetting.mockReturnValue({ mutate, isPending: false, error: null })
  })

  it('renders accessible users, settings, and audit log tabs', async () => {
    const user = userEvent.setup()
    renderPage()

    expect(
      screen.getByRole('heading', { name: /администрирование|administration/i }),
    ).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /пользователи|users/i })).toBeInTheDocument()
    expect(screen.getByText('admin@example.test')).toBeInTheDocument()

    await user.click(screen.getByRole('tab', { name: /настройки инстанса|instance settings/i }))
    expect(screen.getByText('instance.name')).toBeInTheDocument()

    await user.click(screen.getByRole('tab', { name: /журнал аудита|audit log/i }))
    expect(screen.getByText('user.created')).toBeInTheDocument()
    expect(screen.getByText(/new@example\.test/)).toBeInTheDocument()
  })

  it('shows JSON validation feedback without submitting an invalid setting', async () => {
    const user = userEvent.setup()
    renderPage()

    await user.click(screen.getByRole('tab', { name: /настройки инстанса|instance settings/i }))
    await user.type(screen.getByLabelText(/ключ настройки|setting key/i), 'instance.name')
    await user.clear(screen.getByLabelText(/значение json|json value/i))
    fireEvent.change(screen.getByLabelText(/значение json|json value/i), {
      target: { value: '{invalid' },
    })
    await user.click(screen.getByRole('button', { name: /сохранить настройку|save setting/i }))

    expect(screen.getByRole('alert')).toHaveTextContent(/корректный json|valid json/i)
    expect(mutate).not.toHaveBeenCalled()
  })

  it('collects a password and system-admin choice when creating a user', async () => {
    const user = userEvent.setup()
    renderPage()

    await user.click(screen.getByRole('button', { name: /создать пользователя|create user/i }))
    fireEvent.change(screen.getByLabelText(/email/i), { target: { value: 'new@example.test' } })
    fireEvent.change(screen.getByLabelText(/имя пользователя|username/i), {
      target: { value: 'new-user' },
    })
    fireEvent.change(screen.getByLabelText(/отображаемое имя|display name/i), {
      target: { value: 'New User' },
    })
    fireEvent.change(screen.getByLabelText(/пароль|password/i), {
      target: { value: 'safe-password' },
    })
    fireEvent.click(screen.getByLabelText(/системный администратор|system administrator/i))
    await user.click(screen.getByRole('button', { name: /создать пользователя|create user/i }))

    expect(mutate).toHaveBeenCalledWith(
      {
        email: 'new@example.test',
        username: 'new-user',
        display_name: 'New User',
        password: 'safe-password',
        is_system_admin: true,
      },
      expect.any(Object),
    )
  })
})
