import { NamespaceLink as Link } from '@sdlc/ui/ui'

import { withNamespaceLocation } from '@sdlc/ui/lib'
import { useNamespaceContext } from '@/widgets/namespace-context'
export function NamespacePage() {
  const { ref, malformed, query } = useNamespaceContext()
  if (malformed)
    return (
      <p role="alert" className="text-danger">
        Некорректная ссылка на проект.
      </p>
    )
  if (!ref)
    return (
      <div className="space-y-3">
        <h1 className="text-xl font-semibold">Задачи проекта</h1>
        <p>Выберите проект в верхней панели.</p>
        <Link className="text-accent" to="/projects">
          Каталог проектов Tracker
        </Link>
      </div>
    )
  if (query.isPending) return <p role="status">Разрешаем привязку проекта…</p>
  if (query.isError || !query.data)
    return (
      <p role="alert" className="text-danger">
        Привязка Tracker недоступна. Другой проект не выбран.
      </p>
    )
  const context = query.data
  return (
    <div className="space-y-5">
      <h1 className="text-xl font-semibold">{context.label}</h1>
      <p className="text-sm text-text-muted">
        {context.binding.state === 'archived'
          ? 'Проект в архиве. История доступна для чтения.'
          : 'Задачи проекта'}
      </p>
      <div className="flex flex-wrap gap-4">
        <Link
          className="text-accent"
          to={withNamespaceLocation(
            `/projects/${encodeURIComponent(context.resource_key)}/board`,
            ref,
          )}
        >
          Доска
        </Link>
        <Link
          className="text-accent"
          to={withNamespaceLocation(
            `/projects/${encodeURIComponent(context.resource_key)}/backlog`,
            ref,
          )}
        >
          Бэклог
        </Link>
        {context.binding.state === 'active' && (
          <Link
            className="text-accent"
            to={withNamespaceLocation(
              `/issues/create?project_key=${encodeURIComponent(context.resource_key)}`,
              ref,
            )}
          >
            Создать задачу
          </Link>
        )}
      </div>
    </div>
  )
}
