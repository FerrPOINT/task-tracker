import { useLocation } from 'react-router'
import { useProjectDetails } from '@/shared/api/hooks'
import { useVisibleProjects } from '@/shared/api/useVisibleProjects'
import { readSpaceLocation } from '@/shared/lib/space-location'
import { useActiveProjects } from './active-projects'

export const PROJECT_CHOICE_PAGE_SIZE = 20

/** Product catalogs and exact reads retain their existing namespace/space admission. */
export function useProjectChoiceCatalog(selectedKey: string, offset: number, search: string) {
  const v2 = import.meta.env.VITE_SPACE_ACCESS_ENABLED === 'true'
  const context = readSpaceLocation(useLocation().search)
  const projects = useActiveProjects(offset, PROJECT_CHOICE_PAGE_SIZE + 1, search, {
    visibleLimit: PROJECT_CHOICE_PAGE_SIZE,
  })
  const listed = projects.isError
    ? undefined
    : projects.data?.find((project) => project.key === selectedKey)
  const needsExact = Boolean(selectedKey && !listed)
  const legacy = useProjectDetails(!v2 && needsExact ? selectedKey : '')
  const exact = useVisibleProjects(
    { space: context.space, projectKey: selectedKey || undefined, limit: 1, workingOnly: true },
    v2 && needsExact && !context.invalid,
  )
  const detail = v2
    ? !exact.unavailable && !exact.isError
      ? exact.data?.projects.find((project) => project.key === selectedKey)
      : undefined
    : !legacy.isError && legacy.data?.key === selectedKey
      ? legacy.data
      : undefined
  const selected = listed ?? (detail && projects.projectIsActive(detail.id) ? detail : undefined)
  const selectedPending =
    needsExact &&
    !projects.isError &&
    (!v2 || (!context.invalid && !exact.unavailable)) &&
    (projects.isPending || (v2 ? exact.isPending : legacy.isPending))
  const selectedUnavailable = Boolean(selectedKey && !selected && !selectedPending)
  return {
    ...projects,
    selected,
    selectedPending,
    selectedUnavailable,
    searchable: v2,
    retrySelected: () =>
      Promise.all([
        projects.refetch(),
        needsExact ? (v2 ? exact.refetch() : legacy.refetch()) : undefined,
      ]),
  }
}
