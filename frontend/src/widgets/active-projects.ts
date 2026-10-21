import { useProjects } from '@/shared/api/hooks'
import { isUnarchivedProject, useNamespaceCatalog } from './namespace-context'

/** Keep the full ownership catalog; only working lists exclude archived bindings. */
export function useActiveProjects(offset = 0, limit = 100) {
  const projects = useProjects(offset, limit)
  const enabled = import.meta.env.VITE_NAMESPACE_ENABLED === 'true'
  const catalog = useNamespaceCatalog(enabled)
  const archived = new Set(
    catalog.data
      ?.filter((context) => !isUnarchivedProject(context))
      .map((context) => context.binding.resource.resource_id),
  )
  return {
    ...projects,
    unfilteredCount: projects.data?.length ?? 0,
    data:
      enabled && !catalog.isSuccess
        ? undefined
        : projects.data?.filter((project) => !archived.has(project.id)),
    isLoading: projects.isLoading || (enabled && catalog.isPending),
    isPending: projects.isPending || (enabled && catalog.isPending),
    isError: projects.isError || (enabled && catalog.isError),
    error: projects.error ?? (enabled ? catalog.error : null),
    refetch: async () => {
      const [result] = await Promise.all([
        projects.refetch(),
        enabled ? catalog.refetch() : undefined,
      ])
      return result
    },
  }
}
