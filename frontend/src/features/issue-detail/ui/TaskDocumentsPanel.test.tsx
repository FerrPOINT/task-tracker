import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'
import { TaskDocumentsPanel } from './TaskDocumentsPanel'
const ref = {
  registry_instance_id: '11111111-1111-4111-8111-111111111111',
  namespace_id: '22222222-2222-4222-8222-222222222222',
}
vi.mock('@sdlc/ui/ui', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@sdlc/ui/ui')>()),
  usePlatformServices: () => ({ services: [{ key: 'wiki', ui_url: 'http://wiki.test' }] }),
}))
vi.mock('@/widgets/namespace-context', () => ({
  useNamespaceContext: () => ({
    ref,
    query: { data: { resource_key: 'PROJECT', binding: { resource: { instance_id: 'tracker' } } } },
  }),
}))
vi.mock('@/api/client', () => ({
  api: {
    GET: async () => ({
      data: [
        { document_id: 'doc', revision_id: 'revision', title: 'Published document', version: 1 },
      ],
    }),
  },
}))
afterEach(cleanup)
it.each([true, false])('preserves all=%s across the revision and project links', async (all) => {
  const search = `?registry_instance_id=${ref.registry_instance_id}&namespace_id=${ref.namespace_id}${all ? '&project_scope=all' : ''}`
  render(
    <MemoryRouter initialEntries={['/issues/task' + search]}>
      <QueryClientProvider
        client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
      >
        <TaskDocumentsPanel taskId="task" projectKey="PROJECT" />
      </QueryClientProvider>
    </MemoryRouter>,
  )
  const revision = await screen.findByRole('link', { name: 'Published document · ревизия 1' })
  const overview = screen.getByRole('link', { name: 'Открыть документы проекта и связать ревизию' })
  for (const link of [revision, overview]) {
    const url = new URL(link.getAttribute('href')!)
    expect(url.searchParams.get('project_scope')).toBe(all ? 'all' : null)
    expect(url.searchParams.get('namespace_id')).toBe(ref.namespace_id)
  }
  expect(new URL(overview.getAttribute('href')!).searchParams.get('task_id')).toBe('task')
  expect(new URL(revision.getAttribute('href')!).pathname).toBe('/documents/doc/revisions/revision')
})
