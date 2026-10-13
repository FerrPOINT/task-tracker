import { Settings, Trash2 } from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useLocation } from 'react-router'
import { withNamespaceLocation, type NamespaceLocation } from '@sdlc/ui/lib'
import { Button, DropdownMenuItem } from '@sdlc/ui/ui'
import { useNamespaceCatalog } from './namespace-context'

type Project = { key: string; name: string; namespace?: NamespaceLocation }
export function ProjectActionItems({
  project,
  onNavigate,
}: {
  project: Project
  onNavigate?: () => void
}) {
  const { t } = useTranslation()
  const { pathname, search } = useLocation()
  const key = encodeURIComponent(project.key)
  const actions = [
    { to: `/projects/${key}/trash`, label: t('trash.title'), icon: Trash2 },
    {
      to: `/projects/${key}/settings`,
      label: t('navigation.settings'),
      icon: Settings,
    },
  ]
  return (
    <>
      {actions.map(({ to, label, icon: Icon }) => (
        <DropdownMenuItem key={to} asChild>
          <Link
            to={
              import.meta.env.VITE_NAMESPACE_ENABLED === 'true'
                ? withNamespaceLocation(to, project.namespace ?? null, search)
                : to
            }
            onClick={onNavigate}
            aria-current={pathname === to ? 'page' : undefined}
          >
            <Icon aria-hidden className="mr-2 h-4 w-4" />
            {label}
          </Link>
        </DropdownMenuItem>
      ))}
    </>
  )
}
export function ProjectCardActionItems({ project }: { project: Project & { id: string } }) {
  return import.meta.env.VITE_NAMESPACE_ENABLED === 'true' ? (
    <BoundCardActionItems project={project} />
  ) : (
    <ProjectActionItems project={project} />
  )
}
export function ProjectCardLink({
  project,
  children,
  className,
}: {
  project: Project & { id: string }
  children: ReactNode
  className?: string
}) {
  return import.meta.env.VITE_NAMESPACE_ENABLED === 'true' ? (
    <BoundCardLink project={project} className={className}>
      {children}
    </BoundCardLink>
  ) : (
    <Link to={`/projects/${encodeURIComponent(project.key)}/board`} className={className}>
      {children}
    </Link>
  )
}
function BoundCardLink({
  project,
  children,
  className,
}: {
  project: Project & { id: string }
  children: ReactNode
  className?: string
}) {
  const catalog = useNamespaceCatalog()
  const { search } = useLocation()
  const matches =
    catalog.data?.filter((context) => context.binding.resource.resource_id === project.id) ?? []
  if (catalog.isPending)
    return (
      <span className={className} aria-busy="true">
        {children}
      </span>
    )
  if (catalog.isError || matches.length > 1)
    return (
      <span className={className}>
        {children}
        <span role="alert">Контекст проекта недоступен</span>
      </span>
    )
  return (
    <Link
      to={withNamespaceLocation(
        `/projects/${encodeURIComponent(project.key)}/board`,
        matches[0]?.binding.namespace ?? null,
        search,
      )}
      className={className}
    >
      {children}
    </Link>
  )
}
function BoundCardActionItems({ project }: { project: Project & { id: string } }) {
  const catalog = useNamespaceCatalog()
  if (catalog.isPending)
    return <DropdownMenuItem disabled>Проверяем контекст проекта…</DropdownMenuItem>
  if (catalog.isError)
    return <DropdownMenuItem disabled>Контекст проекта недоступен</DropdownMenuItem>
  const matches =
    catalog.data?.filter((context) => context.binding.resource.resource_id === project.id) ?? []
  if (matches.length > 1)
    return <DropdownMenuItem disabled>Контекст проекта неоднозначен</DropdownMenuItem>
  return <ProjectActionItems project={{ ...project, namespace: matches[0]?.binding.namespace }} />
}
export function ProjectActions({
  project,
  onNavigate,
}: {
  project: Project
  onNavigate?: () => void
}) {
  const { t } = useTranslation()
  const { pathname, search } = useLocation()
  const to = `/projects/${encodeURIComponent(project.key)}/settings`
  return (
    <Button asChild variant="ghost" size="icon" className="h-11 w-11">
      <Link
        to={
          import.meta.env.VITE_NAMESPACE_ENABLED === 'true'
            ? withNamespaceLocation(to, project.namespace ?? null, search)
            : to
        }
        onClick={onNavigate}
        aria-current={pathname === to || pathname.startsWith(`${to}/`) ? 'page' : undefined}
        aria-label={`${t('navigation.settings')}: ${project.name} · ${project.key}`}
        title={t('navigation.settings')}
      >
        <Settings aria-hidden className="h-4 w-4" />
      </Link>
    </Button>
  )
}
