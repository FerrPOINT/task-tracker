import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'
import { AdminPage } from './index'

const useAdminSettings = vi.hoisted(() => vi.fn())
const useAdminAuditLog = vi.hoisted(() => vi.fn())
const useUpdateAdminSetting = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useAdminSettings,
  useAdminAuditLog,
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
    useUpdateAdminSetting.mockReturnValue({ mutate, isPending: false, error: null })
  })

  it('renders settings and audit without local user management', async () => {
    const user = userEvent.setup()
    renderPage()

    expect(
      screen.getByRole('heading', { name: /администрирование|administration/i }),
    ).toBeInTheDocument()
    expect(screen.queryByRole('tab', { name: /пользователи|users/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /создать пользователя|create user/i })).not.toBeInTheDocument()

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

})
