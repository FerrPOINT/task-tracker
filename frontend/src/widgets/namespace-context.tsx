import { useLocation, useNavigate } from 'react-router'
import { useQuery } from '@tanstack/react-query'
import { NamespacePicker } from '@pdlc/ui/ui'
import type { Project } from '@/api/project'
import { projectNavigationCatalog, projectSelectionValue } from './project-navigation-catalog'
import {
  projectNavigationScope,
  projectIdentity,
  sortProjectNavigation,
  withNamespaceLocation,
} from '@pdlc/ui/lib'
import type { components } from '@/api/generated'
import { apiBaseUrl } from '@/api/client'
import { useAuthStore } from '@/shared/auth/store'
import { isAllProjects } from '@/shared/lib/project-navigation'
export type ResourceContext = components['schemas']['ResourceContextSummary']
export const isUnarchivedProject = (context: ResourceContext) =>
  context.binding.state !== 'archived'
async function get<T>(path: string, signal?: AbortSignal): Promise<T> {
  const token = useAuthStore.getState().token
  const response = await fetch(apiBaseUrl + path, {
    signal,
    headers: token ? { Authorization: `Bearer ${token}` } : {},
  })
  if (!response.ok) throw new Error(`HTTP ${response.status}`)
  return response.json() as Promise<T>
}
export function useNamespaceContext(enabled = true) {
  const location = useLocation()
  const { ref, malformed } = projectNavigationScope(location.search)
  const query = useQuery({
    queryKey: ['namespace-context', ref?.registry_instance_id, ref?.namespace_id],
    enabled: enabled && Boolean(ref) && !malformed,
    queryFn: ({ signal }) =>
      get<ResourceContext>(
        `/api/v1/namespace-contexts/${ref!.registry_instance_id}/${ref!.namespace_id}`,
        signal,
      ),
  })
  return { ref, malformed, query }
}
export function useNamespaceCatalog(enabled = true) {
  return useQuery({
    queryKey: ['namespace-contexts', 'task-tracker'],
    enabled,
    queryFn: async ({ signal }) => {
      const items: ResourceContext[] = []
      for (let offset = 0; ; offset += 100) {
        const page = await get<ResourceContext[]>(
          `/api/v1/namespace-contexts?limit=100&offset=${offset}`,
          signal,
        )
        items.push(...page)
        if (page.length < 100)
          return sortProjectNavigation(
            items,
            (item) => item.label,
            (item) => projectIdentity(item.binding.namespace),
          )
      }
    },
  })
}
export function NamespaceShellContext({
  projects = [],
  projectKey,
  projectPending = false,
  projectUnavailable = false,
}: {
  projects?: readonly Project[]
  projectKey?: string
  projectPending?: boolean
  projectUnavailable?: boolean
}) {
  const location = useLocation()
  const navigate = useNavigate()
  const { ref, malformed, query } = useNamespaceContext()
  const allProjects = !malformed && isAllProjects(location.search)
  const catalog = useNamespaceCatalog()
  const items = [...(catalog.data ?? [])]
  if (
    query.data &&
    !items.some(
      (item) =>
        item.binding.namespace.namespace_id === query.data.binding.namespace.namespace_id &&
        item.binding.namespace.registry_instance_id ===
          query.data.binding.namespace.registry_instance_id,
    )
  )
    items.push(query.data)
  const navigation = projectNavigationCatalog(projects, items)
  const selectedProject = navigation.find((project) => project.key === projectKey)
  const value = malformed
    ? 'invalid'
    : selectedProject
      ? projectSelectionValue(selectedProject)
      : allProjects
        ? ''
        : ref
          ? `${ref.registry_instance_id}/${ref.namespace_id}`
          : ''
  return (
    <NamespacePicker
      projectIcons
      value={value}
      loading={catalog.isPending || projectPending}
      unavailable={catalog.isError || projectUnavailable}
      options={navigation.map((project) => ({
        value: projectSelectionValue(project),
        label: `${project.name} · ${project.key}`,
        projectKey: project.key,
      }))}
      onChange={(next) => {
        if (!next) {
          navigate(withNamespaceLocation('/projects?project_scope=all', null))
          return
        }
        const project = navigation.find((item) => projectSelectionValue(item) === next)
        if (!project) return
        navigate(
          withNamespaceLocation(
            project.namespace ? '/namespace' : `/projects/${encodeURIComponent(project.key)}/board`,
            project.namespace ?? null,
          ),
        )
      }}
    />
  )
}
