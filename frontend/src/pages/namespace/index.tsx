import {
  PageHeader,
  ProjectAvatar,
  ErrorState,
  LoadingState,
  TextLink,
  SectionNavigation,
  SectionNavigationItem,
} from '@pdlc/ui/ui'
import { ProjectLink as Link } from '@/shared/lib/project-navigation'

import { withNamespaceLocation } from '@pdlc/ui/lib'
import { useNamespaceContext } from '@/widgets/namespace-context'
export function NamespacePage() {
  const { ref, malformed, query } = useNamespaceContext()
  if (malformed) return <ErrorState message="Некорректная ссылка на проект." />
  if (!ref)
    return (
      <div className="space-y-3">
        <PageHeader title={'Задачи проекта'} context={<>Выберите проект в верхней панели.</>} />
        <TextLink asChild>
          <Link to="/projects">Каталог проектов Tracker</Link>
        </TextLink>
      </div>
    )
  if (query.isPending) return <LoadingState message="Разрешаем привязку проекта…" />
  if (query.isError || !query.data)
    return <ErrorState message="Привязка Tracker недоступна. Другой проект не выбран." />
  const context = query.data
  return (
    <div className="space-y-5">
      <PageHeader
        title={context.label}
        leading={<ProjectAvatar projectKey={context.resource_key} />}
        context={
          context.binding.state === 'archived'
            ? 'Проект в архиве. История доступна для чтения.'
            : 'Задачи проекта'
        }
      />
      <SectionNavigation aria-label="Разделы проекта">
        <SectionNavigationItem asChild>
          <Link
            to={withNamespaceLocation(
              `/projects/${encodeURIComponent(context.resource_key)}/board`,
              ref,
            )}
          >
            Доска
          </Link>
        </SectionNavigationItem>
        <SectionNavigationItem asChild>
          <Link
            to={withNamespaceLocation(
              `/projects/${encodeURIComponent(context.resource_key)}/backlog`,
              ref,
            )}
          >
            Бэклог
          </Link>
        </SectionNavigationItem>
        {context.binding.state === 'active' && (
          <SectionNavigationItem asChild>
            <Link
              to={withNamespaceLocation(
                `/issues/create?project_key=${encodeURIComponent(context.resource_key)}`,
                ref,
              )}
            >
              Создать задачу
            </Link>
          </SectionNavigationItem>
        )}
      </SectionNavigation>
    </div>
  )
}
