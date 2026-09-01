import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { describe, expect, it, vi } from 'vitest'
import type { ReactNode } from 'react'
import { useCreateIssueLink } from './hooks'
import { createIssueLink } from '@/api/link'

vi.mock('@/api/link', () => ({
  listIssueLinks: vi.fn(),
  createIssueLink: vi.fn(),
  deleteIssueLink: vi.fn(),
}))

function wrapper(client: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
}

describe('shared api hooks', () => {
  it('invalidates both source and target issue link lists after creating a link', async () => {
    vi.mocked(createIssueLink).mockResolvedValue({
      id: 'link-1',
      source_id: 'issue-source',
      source_key: 'TT-1',
      target_id: 'issue-target',
      target_key: 'TT-2',
      link_type: 'relates',
    })
    const client = new QueryClient()
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    const { result } = renderHook(() => useCreateIssueLink('issue-source'), {
      wrapper: wrapper(client),
    })

    await act(async () => {
      await result.current.mutateAsync({ targetKey: 'TT-2', linkType: 'relates' })
    })

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue-links', 'issue-source'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue-links', 'issue-target'] })
  })
})
