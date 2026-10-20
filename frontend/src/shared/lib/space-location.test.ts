import { describe, expect, it } from 'vitest'
import {
  readSpaceLocation,
  spaceContextKey,
  spaceProjectLocation,
  withSpaceContext,
  workspaceContextKey,
} from './space-location'
const registry = '00000000-0000-4000-8000-000000000001'
const space = { registry_instance_id: registry, space_id: '00000000-0000-4000-8000-000000000002' }
const search = '?' + new URLSearchParams(space)
describe('explicit SpaceRef navigation', () => {
  it('allows no space and retains the pair without deriving a project', () => {
    expect(readSpaceLocation('')).toEqual({ space: null, invalid: false })
    expect(readSpaceLocation(search)).toEqual({ space, invalid: false })
    expect(spaceProjectLocation(space)).toBe('/projects' + search)
    expect(spaceProjectLocation(space, '-')).toBe('/projects/-/board' + search)
  })
  it.each([
    '?space_id=' + space.space_id,
    '?registry_instance_id=' + registry,
    search + '&space_id=' + space.space_id,
    search + '&registry_instance_id=' + registry,
    search + '&namespace_id=legacy',
    '?namespace_id=legacy',
    '?registry_instance_id=bad&space_id=' + space.space_id,
  ])('refuses a partial, duplicated or legacy context %s', (value) => {
    expect(readSpaceLocation(value)).toEqual({ space: null, invalid: true })
    expect(spaceContextKey(value)).toBe('invalid')
  })
  it('preserves destination refs, fragments and ordinary query args', () => {
    expect(withSpaceContext('/projects?offset=3#list', search)).toBe(
      '/projects?offset=3&' + search.slice(1) + '#list',
    )
    expect(
      withSpaceContext({ pathname: '/search', search: '?q=test', hash: '#a' }, search),
    ).toEqual({ pathname: '/search', search: '?q=test&' + search.slice(1), hash: '#a' })
    for (const target of [
      '/projects?namespace_id=old',
      '/projects?space_id=explicit',
      'https://other.example.test/path',
    ])
      expect(withSpaceContext(target, search)).toBe(target)
  })
  it('does not inherit an invalid context and canonicalizes only context identity casing', () => {
    expect(withSpaceContext('/projects', '?namespace_id=old')).toBe('/projects')
    const hex = { ...space, registry_instance_id: '00000000-0000-4000-8000-00000000000a' }
    expect(
      spaceContextKey(
        '?' +
          new URLSearchParams({
            ...hex,
            registry_instance_id: hex.registry_instance_id.toUpperCase(),
          }),
      ),
    ).toBe(spaceContextKey('?' + new URLSearchParams(hex)))
  })
})

it('tracks explicit project/issue identity while ignoring tabs and asynchronous project resolution', () => {
  expect(workspaceContextKey('/projects/QA/board', search)).toBe(
    workspaceContextKey('/projects/QA/settings', search),
  )
  expect(workspaceContextKey('/projects/QA/board', search)).not.toBe(
    workspaceContextKey('/projects/OTHER/board', search),
  )
  expect(workspaceContextKey('/reports', search + '&project_key=QA&tab=burndown')).toBe(
    workspaceContextKey('/projects/QA/board', search),
  )
  expect(workspaceContextKey('/issues/one', search)).not.toBe(
    workspaceContextKey('/issues/two', search),
  )
  expect(workspaceContextKey('/issues/one', search + '&tab=history')).toBe(
    workspaceContextKey('/issues/one', search),
  )
})
