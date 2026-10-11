import { describe, expect, it, vi } from 'vitest'
import {
  fetchNamespaceCatalog,
  type NamespaceContextPage,
  type ResourceContext,
} from './namespace-context'

function item(id: number): ResourceContext {
  return {
    binding: {
      schema_version: 1,
      namespace: { registry_instance_id: `registry-${id}`, namespace_id: `namespace-${id}` },
      resource: {
        kind: 'tracker_project',
        instance_id: `instance-${id}`,
        resource_id: `project-${id}`,
      },
      operation_id: `operation-${id}`,
      generation: 1,
      state: 'active',
      drained: true,
    },
    label: `Project ${id}`,
    resource_key: `P${id}`,
  } as ResourceContext
}

describe('fetchNamespaceCatalog', () => {
  it('continues after a sparse authorized page using the source cursor', async () => {
    const loadPage = vi
      .fn<(...args: [number]) => Promise<NamespaceContextPage>>()
      .mockResolvedValueOnce({ items: [item(1), item(2)], next_offset: 100 })
      .mockResolvedValueOnce({ items: [item(3)], next_offset: null })

    const result = await fetchNamespaceCatalog(loadPage)

    expect(loadPage.mock.calls.map(([offset]) => offset)).toEqual([0, 100])
    expect(result).toHaveLength(3)
  })

  it('continues after a full accessible page and stops at the source end', async () => {
    const loadPage = vi
      .fn<(...args: [number]) => Promise<NamespaceContextPage>>()
      .mockResolvedValueOnce({
        items: Array.from({ length: 100 }, (_, index) => item(index)),
        next_offset: 100,
      })
      .mockResolvedValueOnce({
        items: Array.from({ length: 38 }, (_, index) => item(index + 100)),
        next_offset: null,
      })

    const result = await fetchNamespaceCatalog(loadPage)

    expect(loadPage.mock.calls.map(([offset]) => offset)).toEqual([0, 100])
    expect(result).toHaveLength(138)
  })
})
