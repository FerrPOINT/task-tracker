import {ResourceList, ResourceListRow, LoadingState, usePlatformServices} from '@pdlc/ui/ui'
import { useQuery } from '@tanstack/react-query'
import { useLocation } from 'react-router'

import { withNamespaceLocation } from '@pdlc/ui/lib'
import { api } from '@/api/client'
import { useNamespaceContext } from '@/widgets/namespace-context'

export function TaskDocumentsPanel({ taskId, projectKey }: { taskId: string; projectKey: string }) {
  const { search } = useLocation()
  const { ref, query: context } = useNamespaceContext()
  const { services } = usePlatformServices()
  const matching = context.data?.resource_key === projectKey
  const links = useQuery({
    queryKey: ['task-documents', ref?.registry_instance_id, ref?.namespace_id, taskId],
    enabled: Boolean(ref && matching),
    queryFn: async ({ signal }) => {
      const { data, error } = await api.GET('/api/v1/tasks/{id}/documents', {
        params: { path: { id: taskId } },
        signal,
      })
      if (error || !data) throw new Error('Wiki недоступна')
      return data
    },
  })
  if (!ref) return null
  const wiki = services.find((service) => service.key === 'wiki')?.ui_url
  return (
    <section className="space-y-3 rounded-lg border border-border bg-surface p-4">
      <h2 className="font-semibold">Документы задачи</h2>
      {context.isPending ? (
        <p role="status">Проверяем контекст задачи…</p>
      ) : !matching ? (
        <p role="alert" className="text-danger">
          Задача не подтверждена в выбранном проекте.
        </p>
      ) : links.isPending ? (
        <LoadingState message="Загружаем связи Wiki…" />
      ) : links.isError ? (
        <p role="alert" className="text-danger">
          Wiki недоступна. Список документов не получен.
        </p>
      ) : (
        <ResourceList asChild>
          <ul>
            {links.data?.map((link) => (
              <ResourceListRow asChild key={link.revision_id}>
                <li>
                  {wiki ? (
                    <a
                      className="text-accent hover:underline"
                      href={withNamespaceLocation(
                        `${wiki}/documents/${link.document_id}/revisions/${link.revision_id}`,
                        ref,
                        search,
                      )}
                    >
                      {link.title} · ревизия {link.version}
                    </a>
                  ) : (
                    <span>
                      {link.title} · ревизия {link.version}
                    </span>
                  )}
                </li>
              </ResourceListRow>
            ))}
            {links.data?.length === 0 && (
              <ResourceListRow asChild>
                <li className="text-text-muted">
                  Опубликованные ревизии пока не связаны с задачей.
                </li>
              </ResourceListRow>
            )}
          </ul>
        </ResourceList>
      )}
      {wiki && matching && (
        <a
          className="inline-block text-sm text-accent hover:underline"
          href={withNamespaceLocation(
            `${wiki}/namespace?task_id=${taskId}&tracker_instance_id=${context.data!.binding.resource.instance_id}`,
            ref,
            search,
          )}
        >
          Открыть документы проекта и связать ревизию
        </a>
      )}
    </section>
  )
}
