import type { To } from 'react-router'
import { spaceKey, spaceRefSchema, type SpaceRef } from '@/api/space-catalog'

export function readSpaceLocation(search: string): { space: SpaceRef | null; invalid: boolean } {
  const params = new URLSearchParams(search)
  const registry = params.getAll('registry_instance_id'),
    space = params.getAll('space_id')
  if (!registry.length && !space.length && !params.has('namespace_id'))
    return { space: null, invalid: false }
  const parsed = spaceRefSchema.safeParse({ registry_instance_id: registry[0], space_id: space[0] })
  if (params.has('namespace_id') || registry.length !== 1 || space.length !== 1 || !parsed.success)
    return { space: null, invalid: true }
  return { space: parsed.data, invalid: false }
}
export function spaceContextKey(search: string): string {
  const value = readSpaceLocation(search)
  return value.invalid ? 'invalid' : value.space ? spaceKey(value.space) : 'none'
}
export function spaceProjectLocation(space: SpaceRef | null, projectKey?: string): string {
  const path = projectKey ? '/projects/' + encodeURIComponent(projectKey) + '/board' : '/projects'
  return path + (space ? '?' + new URLSearchParams(space).toString() : '')
}
/** Inherit an explicit context without replacing one specified by the destination. */
export function withSpaceContext(to: To, search: string): To {
  const current = readSpaceLocation(search)
  if (!current.space || current.invalid) return to
  if (typeof to === 'string' && /^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(to)) return to
  const parts = typeof to === 'string' ? to.match(/^([^?#]*)(\?[^#]*)?(#.*)?$/) : null
  if (typeof to === 'string' && !parts) return to
  const params = new URLSearchParams(typeof to === 'string' ? parts?.[2] : to.search)
  if (['space_id', 'registry_instance_id', 'namespace_id'].some((key) => params.has(key))) return to
  params.set('registry_instance_id', current.space.registry_instance_id)
  params.set('space_id', current.space.space_id)
  return typeof to === 'string'
    ? (parts?.[1] ?? '') + '?' + params.toString() + (parts?.[3] ?? '')
    : { ...to, search: '?' + params.toString() }
}
