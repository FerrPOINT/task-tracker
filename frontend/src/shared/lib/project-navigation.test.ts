import { describe, expect, it } from 'vitest'
import { isAllProjects, projectNavigationTarget } from './project-navigation'

describe('project catalog filter transport', () => {
  it('distinguishes the catalog filter from the open resource context', () => {
    expect(isAllProjects('')).toBe(true)
    expect(isAllProjects('?namespace_id=n&registry_instance_id=r')).toBe(false)
    expect(isAllProjects('?namespace_id=n&registry_instance_id=r&project_scope=all')).toBe(true)
    expect(isAllProjects('?namespace_id=broken')).toBe(false)
    expect(isAllProjects('?namespace_id=n&project_scope=all&project_scope=selected')).toBe(false)
  })

  it('preserves resource refs, task tabs, hashes and object targets', () => {
    expect(projectNavigationTarget('/issues/id?tab=worklog#entry', '?project_scope=all')).toBe(
      '/issues/id?tab=worklog&project_scope=all#entry',
    )
    expect(
      projectNavigationTarget('/projects/B/board?namespace_id=b&registry_instance_id=r', ''),
    ).toBe('/projects/B/board?namespace_id=b&registry_instance_id=r&project_scope=all')
    expect(
      projectNavigationTarget(
        { pathname: '/reports', search: '?project_key=B', hash: '#chart' },
        '',
      ),
    ).toEqual({ pathname: '/reports', search: '?project_key=B&project_scope=all', hash: '#chart' })
    expect(projectNavigationTarget('/namespace?project_scope=selected', '?project_scope=all')).toBe(
      '/namespace?project_scope=selected',
    )
  })

  it('keeps selected-project URLs and external destinations unchanged', () => {
    expect(
      projectNavigationTarget('/projects/A/trash', '?namespace_id=a&registry_instance_id=r'),
    ).toBe('/projects/A/trash')
    for (const target of [
      'https://wiki.test/namespace',
      '//forge.test/repos',
      'mailto:operator@example.test',
    ])
      expect(projectNavigationTarget(target, '')).toBe(target)
  })
})
