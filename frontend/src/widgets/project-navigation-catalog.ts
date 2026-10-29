import { projectIdentity, sortProjectNavigation, type NamespaceLocation } from '@pdlc/ui/lib'
import type { Project } from '@/api/project'
import type { ResourceContext } from './namespace-context'

export type NavigationProject = {
  id: string
  key: string
  name: string
  namespace?: NamespaceLocation
}

/** Confirmed bindings supply context and lifecycle, not catalogue membership. */
export function projectNavigationCatalog(
  projects: readonly Pick<Project, 'id' | 'key' | 'name'>[],
  contexts: readonly ResourceContext[],
): NavigationProject[] {
  const bound = new Set(contexts.map((context) => context.binding.resource.resource_id))
  const items: NavigationProject[] = contexts
    .filter((context) => context.binding.state !== 'archived')
    .map((context) => ({
      id: `${context.binding.resource.instance_id}/${context.binding.resource.resource_id}`,
      key: context.resource_key,
      name: context.label,
      namespace: context.binding.namespace,
    }))
  for (const project of projects) {
    if (!bound.has(project.id)) items.push({ id: project.id, key: project.key, name: project.name })
  }
  return sortProjectNavigation(
    items,
    (item) => item.name,
    (item) => item.id,
  )
}

export function projectSelectionValue(project: NavigationProject) {
  return project.namespace ? projectIdentity(project.namespace) : `tracker-project/${project.id}`
}
