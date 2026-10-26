import { useApplicationAccess } from '@pdlc/ui/auth'
import { ProjectLink } from '@/shared/lib/project-navigation'
import { withSpaceContext } from '@/shared/lib/space-location'
import { Settings, Trash2 } from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useLocation } from 'react-router'
import { withNamespaceLocation, type NamespaceLocation } from '@pdlc/ui/lib'
import {
  Button,
  DropdownMenuItem,
  Notice,
  ResourceListRow,
  ResourceListDescription,
  usePlatformServices,
} from '@pdlc/ui/ui'
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
  const canAdmin = useApplicationAccess()?.permits([], true) === true
  const actions = [
    { to: `/projects/${key}/trash`, label: t('trash.title'), icon: Trash2, administrative: false },
    {
      to: `/projects/${key}/settings`,
      label: t('navigation.settings'),
      icon: Settings,
      administrative: true,
    },
  ]
  return (
    <>
      {actions
        .filter((item) => canAdmin || !item.administrative)
        .map(({ to, label, icon: Icon }) => (
          <DropdownMenuItem key={to} asChild>
            <Link
              to={
                import.meta.env.VITE_NAMESPACE_ENABLED === 'true' &&
                import.meta.env.VITE_SPACE_ACCESS_ENABLED !== 'true'
                  ? withNamespaceLocation(to, project.namespace ?? null, search)
                  : import.meta.env.VITE_SPACE_ACCESS_ENABLED === 'true'
                    ? withSpaceContext(to, search)
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
function DeleteProjectItem({ onDelete }: { onDelete: () => void }) {
  const { t } = useTranslation()
  return (
    <DropdownMenuItem onClick={onDelete} tone="danger" className="gap-2">
      <Trash2 className="h-4 w-4" />
      {t('common.delete')}
    </DropdownMenuItem>
  )
}
export function ProjectCardActionItems({
  project,
  onDelete,
}: {
  project: Project & { id: string }
  onDelete?: () => void
}) {
  const canAdmin = useApplicationAccess()?.permits([], true) === true
  return import.meta.env.VITE_NAMESPACE_ENABLED === 'true' &&
    import.meta.env.VITE_SPACE_ACCESS_ENABLED !== 'true' ? (
    <BoundCardActionItems project={project} onDelete={onDelete} />
  ) : (
    <>
      <ProjectActionItems project={project} />
      {canAdmin && onDelete && <DeleteProjectItem onDelete={onDelete} />}
    </>
  )
}
export function ProjectCardState({ projectId }: { projectId: string }) {
  return import.meta.env.VITE_NAMESPACE_ENABLED === 'true' &&
    import.meta.env.VITE_SPACE_ACCESS_ENABLED !== 'true' ? (
    <BoundCardState projectId={projectId} />
  ) : null
}
function BoundCardState({ projectId }: { projectId: string }) {
  const catalog = useNamespaceCatalog()
  const { t } = useTranslation()
  const matches = catalog.data?.filter(
    (context) => context.binding.resource.resource_id === projectId,
  )
  const context = matches?.length === 1 ? matches[0] : undefined
  if (catalog.isPending || catalog.isError || !context) return null
  return context.binding.state === 'archived' ? (
    <ResourceListDescription asChild>
      <span className="block">{t('projects.archived')}</span>
    </ResourceListDescription>
  ) : null
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
  return import.meta.env.VITE_NAMESPACE_ENABLED === 'true' &&
    import.meta.env.VITE_SPACE_ACCESS_ENABLED !== 'true' ? (
    <BoundCardLink project={project} className={className}>
      {children}
    </BoundCardLink>
  ) : (
    <ResourceListRow asChild interactive>
      <ProjectLink to={`/projects/${encodeURIComponent(project.key)}/board`} className={className}>
        {children}
      </ProjectLink>
    </ResourceListRow>
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
      <ResourceListRow className={className} aria-busy="true">
        {children}
      </ResourceListRow>
    )
  if (catalog.isError || matches.length > 1)
    return (
      <ResourceListRow className={className}>
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <div className="flex min-w-0 items-center gap-3">{children}</div>
          <Notice role="alert" tone="danger" appearance="inline">
            Контекст проекта недоступен
          </Notice>
        </div>
      </ResourceListRow>
    )
  return (
    <ResourceListRow asChild interactive>
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
    </ResourceListRow>
  )
}
function BoundCardActionItems({
  project,
  onDelete,
}: {
  project: Project & { id: string }
  onDelete?: () => void
}) {
  const catalog = useNamespaceCatalog()
  const { services } = usePlatformServices()
  const canAdmin = useApplicationAccess()?.permits([], true) === true
  const { t } = useTranslation()
  if (catalog.isPending)
    return <DropdownMenuItem disabled>Проверяем контекст проекта…</DropdownMenuItem>
  if (catalog.isError)
    return <DropdownMenuItem disabled>Контекст проекта недоступен</DropdownMenuItem>
  const matches =
    catalog.data?.filter((context) => context.binding.resource.resource_id === project.id) ?? []
  if (matches.length > 1)
    return <DropdownMenuItem disabled>Контекст проекта неоднозначен</DropdownMenuItem>
  const context = matches[0]
  const admin = services.find((service) => service.key === 'admin-panel')?.ui_url
  // A confirmed binding makes lifecycle an Admin operation. Tracker's delete
  // endpoint deliberately rejects it, including archived projects.
  const manageUrl =
    context && admin
      ? withNamespaceLocation(
          `${admin.replace(/\/+$/, '')}/namespaces/${encodeURIComponent(context.binding.namespace.namespace_id)}`,
          context.binding.namespace,
        )
      : null
  return (
    <>
      <ProjectActionItems project={{ ...project, namespace: context?.binding.namespace }} />
      {canAdmin &&
        (context ? (
          manageUrl ? (
            <DropdownMenuItem asChild>
              <a href={manageUrl}>
                <Settings aria-hidden className="mr-2 h-4 w-4" />
                {t('projects.manage')}
              </a>
            </DropdownMenuItem>
          ) : (
            <DropdownMenuItem disabled>{t('projects.managementUnavailable')}</DropdownMenuItem>
          )
        ) : (
          onDelete && <DeleteProjectItem onDelete={onDelete} />
        ))}
    </>
  )
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
  const canAdmin = useApplicationAccess()?.permits([], true) === true
  const to = `/projects/${encodeURIComponent(project.key)}/settings`
  if (!canAdmin) return null
  return (
    <Button asChild variant="ghost" size="icon">
      <Link
        to={
          import.meta.env.VITE_NAMESPACE_ENABLED === 'true' &&
          import.meta.env.VITE_SPACE_ACCESS_ENABLED !== 'true'
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
