import { useCallback } from 'react'
import {
  useLocation,
  type LinkProps,
  type NavigateFunction,
  type NavigateOptions,
  type To,
} from 'react-router'
import { NamespaceLink, useNamespaceNavigate } from '@sdlc/ui/ui'

export function isAllProjects(search: string) {
  const params = new URLSearchParams(search)
  return (
    (params.getAll('project_scope').length === 1 && params.get('project_scope') === 'all') ||
    (!params.has('namespace_id') && !params.has('registry_instance_id'))
  )
}

export function projectNavigationTarget(to: To, search: string): To {
  if (!isAllProjects(search)) return to
  const parts = typeof to === 'string' ? to.match(/^([^?#]*)(\?[^#]*)?(#.*)?$/) : null
  if (typeof to === 'string' && (!parts || /^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(to))) return to
  const params = new URLSearchParams(typeof to === 'string' ? parts?.[2] : to.search)
  if (!params.has('project_scope')) params.set('project_scope', 'all')
  const query = `?${params}`
  return typeof to === 'string'
    ? `${parts?.[1] ?? ''}${query}${parts?.[3] ?? ''}`
    : { ...to, search: query }
}

/** The catalog filter is independent of the verified Namespace of the open resource. */
export function ProjectLink({ to, ...props }: LinkProps) {
  const { search } = useLocation()
  return (
    <NamespaceLink
      to={
        import.meta.env.VITE_NAMESPACE_ENABLED === 'true' ? projectNavigationTarget(to, search) : to
      }
      {...props}
    />
  )
}

export function useProjectNavigate(): NavigateFunction {
  const navigate = useNamespaceNavigate()
  const { search } = useLocation()
  return useCallback(
    (to: To | number, options?: NavigateOptions) => {
      if (typeof to === 'number') return navigate(to)
      return navigate(
        import.meta.env.VITE_NAMESPACE_ENABLED === 'true'
          ? projectNavigationTarget(to, search)
          : to,
        options,
      )
    },
    [navigate, search],
  ) as NavigateFunction
}
