import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { SpaceSetField } from './SpaceSetField'
import { useAuthStore } from '@/shared/auth/store'
import { acceptSpaceSession, useSpaceSession } from '@/shared/auth/space-session'
import type { SpaceRef, VisibleSpaceCatalog } from '@/api/space-catalog'
const read = vi.hoisted(() => vi.fn())
vi.mock('@/api/space-catalog', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/api/space-catalog')>()),
  getVisibleSpaceCatalog: read,
}))
const registry = '00000000-0000-4000-8000-000000000001'
const ref = (n: number): SpaceRef => ({
  registry_instance_id: registry,
  space_id: '00000000-0000-4000-8000-' + String(n).padStart(12, '0'),
})
const hidden = ref(999)
const clients: QueryClient[] = []
let entries: VisibleSpaceCatalog['spaces']
function Field({
  initial = [hidden],
  disabled = false,
}: {
  initial?: SpaceRef[]
  disabled?: boolean
}) {
  const [value, onChange] = useState(initial)
  return (
    <>
      <SpaceSetField value={value} onChange={onChange} keepNonEmpty disabled={disabled} />
      <output data-testid="selection">{JSON.stringify(value)}</output>
    </>
  )
}
function mount(initial?: SpaceRef[], disabled = false) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } })
  clients.push(client)
  return render(
    <QueryClientProvider client={client}>
      <Field initial={initial} disabled={disabled} />
    </QueryClientProvider>,
  )
}
beforeEach(() => {
  useAuthStore.getState().logout()
  useAuthStore
    .getState()
    .setAuth({ token: 'fixture', userId: 'local', email: 'fixture@example.test' })
  acceptSpaceSession(
    {
      accessToken: 'fixture',
      subject: 'central',
      email: 'fixture@example.test',
      name: 'Fixture',
      expiresAt: Date.now() + 60_000,
      returnTo: '/',
    },
    { id: 'local' },
  )
  entries = Array.from({ length: 11 }, (_, n) => ({
    space: ref(n + 10),
    revision: 1,
    state: 'active',
    metadata: {
      slug: 'space-' + n,
      name: n < 2 ? 'Одинаковое имя' : 'Пространство ' + n,
      description: '',
      responsible_subject: 'central',
    },
  }))
  read.mockReset().mockImplementation(async (identity, query) => {
    const now = Math.floor(Date.now() / 1000)
    const filtered = entries.filter((entry) => entry.metadata.name.includes(query.q))
    return {
      schema_version: 2,
      identity,
      policy_revision: 1,
      validity: { observed_at: now, valid_until: now + 30 },
      spaces: filtered.slice(query.offset, query.offset + query.limit),
      total: filtered.length,
      limit: query.limit,
      offset: query.offset,
      has_more: query.offset + query.limit < filtered.length,
    }
  })
})
afterEach(() => {
  cleanup()
  clients.splice(0).forEach((client) => client.clear())
  useSpaceSession.setState({ session: null })
})
describe('paged multi-space field', () => {
  it('keeps distinct IDs with equal names and retains selections across server pages', async () => {
    const user = userEvent.setup()
    mount()
    const first = await screen.findAllByRole('checkbox', { name: /Одинаковое имя/ })
    expect(first).toHaveLength(2)
    await user.click(first[0]!)
    await user.click(first[1]!)
    await user.click(screen.getByRole('button', { name: 'Далее' }))
    await screen.findByRole('checkbox', { name: /Пространство 10/ })
    expect(JSON.parse(screen.getByTestId('selection').textContent!)).toEqual([
      hidden,
      ref(10),
      ref(11),
    ])
    expect(read.mock.calls.some(([, query]) => query.offset === 10)).toBe(true)
    await user.click(screen.getByRole('button', { name: 'Назад' }))
    await waitFor(() =>
      expect(screen.getAllByRole('checkbox', { name: /Одинаковое имя/ })[0]).toBeChecked(),
    )
    expect(
      screen.getByRole('button', { name: 'Убрать пространство ' + hidden.space_id }),
    ).toBeEnabled()
  })
  it('filters on the server and does not discard an association absent from search results', async () => {
    const user = userEvent.setup()
    mount()
    await screen.findAllByRole('checkbox')
    await user.type(screen.getByRole('searchbox', { name: 'Поиск пространств' }), 'Пространство 10')
    await waitFor(() => expect(screen.getAllByRole('checkbox')).toHaveLength(1))
    expect(JSON.parse(screen.getByTestId('selection').textContent!)).toEqual([hidden])
    expect(read.mock.calls.at(-1)?.[1]).toMatchObject({ q: 'Пространство 10', offset: 0 })
  })
  it('does not add an archived space or permit removing the final association', async () => {
    entries[0]!.state = 'archived'
    mount([hidden])
    const archived = (await screen.findAllByRole('checkbox', { name: /Одинаковое имя/ }))[0]!
    expect(archived).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'Убрать пространство ' + hidden.space_id }),
    ).toBeDisabled()
    expect(JSON.parse(screen.getByTestId('selection').textContent!)).toEqual([hidden])
  })
  it('keeps selection controls read-only while preserving catalog search', async () => {
    mount([hidden, ref(10)], true)
    const items = await screen.findAllByRole('checkbox')
    expect(items.every((item) => (item as HTMLInputElement).disabled)).toBe(true)
    expect(screen.getByRole('searchbox', { name: 'Поиск пространств' })).toBeEnabled()
    expect(
      screen.getByRole('button', { name: 'Убрать пространство ' + hidden.space_id }),
    ).toBeDisabled()
  })
})
