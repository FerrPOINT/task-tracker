import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
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

const mutateAsync = vi.fn()
const refetchSettings = vi.fn()
const refetchAudit = vi.fn()

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
      refetch: refetchSettings,
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
      isFetching: false,
      refetch: refetchAudit,
    })
    mutateAsync.mockResolvedValue({ key: 'instance.name', value: 'Task Tracker' })
    useUpdateAdminSetting.mockReturnValue({ mutateAsync, isPending: false })
  })

  it('renders settings and audit without local user management', async () => {
    const user = userEvent.setup()
    renderPage()

    expect(
      screen.getByRole('heading', { name: /администрирование|administration/i }),
    ).toBeInTheDocument()
    expect(screen.queryByRole('tab', { name: /пользователи|users/i })).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: /создать пользователя|create user/i }),
    ).not.toBeInTheDocument()

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
    expect(mutateAsync).not.toHaveBeenCalled()
  })

  it('rejects a whitespace-only setting key', async () => {
    const user = userEvent.setup()
    renderPage()

    fireEvent.change(screen.getByLabelText(/ключ настройки|setting key/i), {
      target: { value: '   ' },
    })
    await user.click(screen.getByRole('button', { name: /сохранить настройку|save setting/i }))

    expect(screen.getByRole('alert')).toHaveTextContent('Укажите ключ настройки.')
    expect(mutateAsync).not.toHaveBeenCalled()
  })

  it('prefills a setting from its row and confirms a successful save', async () => {
    const user = userEvent.setup()
    renderPage()

    await user.click(screen.getByRole('button', { name: 'Изменить настройку instance.name' }))
    expect(screen.getByLabelText(/ключ настройки|setting key/i)).toHaveValue('instance.name')
    expect(screen.getByLabelText(/значение json|json value/i)).toHaveValue('"Task Tracker"')
    await user.click(screen.getByRole('button', { name: /сохранить настройку|save setting/i }))

    await waitFor(() =>
      expect(mutateAsync).toHaveBeenCalledWith({ key: 'instance.name', value: 'Task Tracker' }),
    )
    expect(await screen.findByRole('status')).toHaveTextContent(
      'Настройка instance.name сохранена.',
    )
  })

  it('keeps the draft after save failure and allows another attempt', async () => {
    mutateAsync.mockRejectedValueOnce(new Error('Bad request'))
    const user = userEvent.setup()
    renderPage()

    await user.click(screen.getByRole('button', { name: 'Изменить настройку instance.name' }))
    await user.click(screen.getByRole('button', { name: /сохранить настройку|save setting/i }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Не удалось сохранить настройку')
    expect(screen.getByLabelText(/значение json|json value/i)).toHaveValue('"Task Tracker"')

    await user.click(screen.getByRole('button', { name: /сохранить настройку|save setting/i }))
    expect(await screen.findByRole('status')).toHaveTextContent('сохранена')
    expect(mutateAsync).toHaveBeenCalledTimes(2)
  })

  it('offers retry for independent settings and audit load failures', async () => {
    useAdminSettings.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('Failed'),
      refetch: refetchSettings,
    })
    useAdminAuditLog.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('Failed'),
      refetch: refetchAudit,
    })
    const user = userEvent.setup()
    renderPage()

    await user.click(screen.getByRole('button', { name: /повторить/i }))
    expect(refetchSettings).toHaveBeenCalledOnce()
    await user.click(screen.getByRole('tab', { name: /журнал аудита|audit log/i }))
    await user.click(screen.getByRole('button', { name: /повторить/i }))
    expect(refetchAudit).toHaveBeenCalledOnce()
  })

  it('requests a larger audit page without changing tabs', async () => {
    useAdminAuditLog.mockReturnValue({
      data: Array.from({ length: 20 }, (_, index) => ({
        id: `a${index}`,
        action: `action.${index}`,
        entity_type: 'issue',
        entity_id: null,
        actor_id: null,
        metadata: {},
        created_at: '2026-08-25T10:00:00Z',
      })),
      isLoading: false,
      isFetching: false,
      error: null,
      refetch: refetchAudit,
    })
    const user = userEvent.setup()
    renderPage()

    await user.click(screen.getByRole('tab', { name: /журнал аудита|audit log/i }))
    await user.click(screen.getByRole('button', { name: /загрузить ещё|load more/i }))
    expect(useAdminAuditLog).toHaveBeenLastCalledWith(40)
  })
})
