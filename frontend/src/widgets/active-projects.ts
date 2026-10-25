import { useProjects } from '@/shared/api/hooks'
import { isUnarchivedProject, useNamespaceCatalog } from './namespace-context'

/** Keep the full ownership catalog; only working lists exclude archived bindings. */
export function useActiveProjects(offset = 0, limit = 100, search = '', visibleLimit = limit) {
  const projects = useProjects(offset, limit, search, true)
  const enabled =
    import.meta.env.VITE_NAMESPACE_ENABLED === 'true' &&
    import.meta.env.VITE_SPACE_ACCESS_ENABLED !== 'true'
  const catalog = useNamespaceCatalog(enabled)
  const archived = new Set(
    catalog.data
      ?.filter((context) => !isUnarchivedProject(context))
      .map((context) => context.binding.resource.resource_id),
  )
  const projectIsActive = (id: string) => (!enabled || catalog.isSuccess) && !archived.has(id)
  const total = projects.catalog?.total
  const rawCount = projects.data?.length
  return {
    ...projects,
    unfilteredCount: projects.data?.length ?? 0,
    projectIsActive,
    hasMore:
      total !== undefined
        ? offset + visibleLimit < total
        : rawCount === undefined
          ? undefined
          : rawCount > visibleLimit
            ? true
            : rawCount < limit
              ? false
              : undefined,
    data:
      enabled && !catalog.isSuccess
        ? undefined
        : projects.data?.slice(0, visibleLimit).filter((project) => projectIsActive(project.id)),
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
