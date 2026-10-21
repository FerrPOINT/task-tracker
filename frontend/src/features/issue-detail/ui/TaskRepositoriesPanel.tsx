import {
  Select,
  ResourceList,
  ResourceListRow,
  LoadingState,
  Button,
  usePlatformServices,
} from '@pdlc/ui/ui'
import { useState } from 'react'
import { useLocation } from 'react-router'
import { useQuery, useQueryClient } from '@tanstack/react-query'

import { withNamespaceLocation } from '@pdlc/ui/lib'
import { api } from '@/api/client'
import { useNamespaceContext } from '@/widgets/namespace-context'

export function TaskRepositoriesPanel({
  taskId,
  projectKey,
}: {
  taskId: string
  projectKey: string
}) {
  const { ref, query: context } = useNamespaceContext()
  const { search } = useLocation()
  const { services } = usePlatformServices()
  const cache = useQueryClient()
  const [offset, setOffset] = useState(0)
  const [selected, setSelected] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [evidenceOffset, setEvidenceOffset] = useState(0)
  const matching = context.data?.resource_key === projectKey
  const key = ['project-repositories', ref?.registry_instance_id, ref?.namespace_id, projectKey]
  const scope = JSON.stringify(key)
  const [linkedPage, setLinkedPage] = useState({ scope, offset: 0 })
  const linkedOffset = linkedPage.scope === scope ? linkedPage.offset : 0
  const linked = useQuery({
    queryKey: [...key, linkedOffset],
    enabled: Boolean(ref && matching),
    queryFn: async ({ signal }) => {
      const { data, error } = await api.GET('/api/v1/projects/{project_key}/repositories', {
        params: { path: { project_key: projectKey }, query: { offset: linkedOffset } },
        signal,
      })
      if (error || !data) throw new Error('Связи репозиториев недоступны')
      return data
    },
  })
  const evidence = useQuery({
    queryKey: [
      'task-delivery-evidence',
      ref?.registry_instance_id,
      ref?.namespace_id,
      taskId,
      evidenceOffset,
    ],
    enabled: Boolean(ref && matching),
    queryFn: async ({ signal }) => {
      const { data, error } = await api.GET('/api/v1/tasks/{id}/delivery-evidence', {
        params: { path: { id: taskId }, query: { offset: evidenceOffset } },
        signal,
      })
      if (error || !data) throw new Error('Свидетельства Forge недоступны')
      return data
    },
  })
  const available = useQuery({
    queryKey: ['available-task-repositories', ...key, offset],
    enabled: Boolean(ref && matching && context.data?.binding.state === 'active'),
    queryFn: async ({ signal }) => {
      const { data, error } = await api.GET(
        '/api/v1/projects/{project_key}/available-repositories',
        {
          params: { path: { project_key: projectKey }, query: { offset } },
          signal,
        },
      )
      if (error || !data) throw new Error('Каталог Forge недоступен')
      return data
    },
  })
  async function link() {
    const repository = available.data?.find(
      (item) => `${item.forge_instance_id}/${item.repository_id}` === selected,
    )
    if (!repository) return
    setBusy(true)
    setError('')
    try {
      const { error } = await api.POST('/api/v1/projects/{project_key}/repositories', {
        params: { path: { project_key: projectKey } },
        body: {
          forge_instance_id: repository.forge_instance_id,
          repository_id: repository.repository_id,
        },
      })
      if (error) throw new Error('Не удалось подтвердить связь')
      await cache.invalidateQueries({ queryKey: key })
    } catch (error) {
      setError(error instanceof Error ? error.message : 'Операция отклонена')
    } finally {
      setBusy(false)
    }
  }
  if (!ref || !matching) return null
  const forge = services.find((service) => service.key === 'ci-cd')?.ui_url
  const fleet = services.find((service) => service.key === 'fleet-control')?.ui_url
  return (
    <section className="space-y-3 rounded-lg border border-border bg-surface p-4">
      <h2 className="font-semibold">Репозитории проекта</h2>
      {fleet && (
        <p>
          <a
            className="text-accent"
            href={withNamespaceLocation(
              `${fleet}/chats?task_id=${taskId}&tracker_instance_id=${context.data!.binding.resource.instance_id}`,
              ref,
            )}
          >
            Чаты Fleet с контекстом задачи
          </a>
        </p>
      )}
      {linked.isPending ? (
        <LoadingState message="Загружаем связи…" />
      ) : linked.isError ? (
        <p role="alert" className="text-danger">
          Связи репозиториев недоступны.
        </p>
      ) : (
        <ResourceList asChild>
          <ul>
            {linked.data?.map((repository) => (
              <ResourceListRow
                asChild
                key={`${repository.forge_instance_id}/${repository.repository_id}`}
              >
                <li>
                  {forge ? (
                    <a
                      className="text-accent"
                      href={withNamespaceLocation(
                        `${forge}/catalog/repositories/${repository.repository_id}?task_id=${taskId}&tracker_instance_id=${context.data!.binding.resource.instance_id}`,
                        ref,
                        search,
                      )}
                    >
                      {repository.public_name}
                    </a>
                  ) : (
                    repository.public_name
                  )}
                </li>
              </ResourceListRow>
            ))}
            {linked.data?.length === 0 && (
              <ResourceListRow asChild>
                <li className="text-text-muted">
                  {linkedOffset === 0
                    ? 'Репозитории пока не указаны.'
                    : 'На этой странице связей нет.'}
                </li>
              </ResourceListRow>
            )}
          </ul>
        </ResourceList>
      )}
      {(linkedOffset > 0 || (!linked.isError && (linked.data?.length ?? 0) === 100)) && (
        <div className="flex gap-2" aria-label="Страницы связей проекта">
          <Button
            variant="outline"
            disabled={linked.isFetching || linkedOffset === 0}
            onClick={() => setLinkedPage({ scope, offset: Math.max(0, linkedOffset - 100) })}
          >
            Предыдущие связи
          </Button>
          <Button
            variant="outline"
            disabled={linked.isFetching || linked.isError || (linked.data?.length ?? 0) < 100}
            onClick={() => setLinkedPage({ scope, offset: linkedOffset + 100 })}
          >
            Следующие связи
          </Button>
        </div>
      )}
      {context.data?.binding.state === 'active' && (
        <div className="space-y-3">
          {available.isError ? (
            <p role="alert" className="text-danger">
              Каталог Forge недоступен. Сохранённые связи остаются выше.
            </p>
          ) : (
            <>
              <label className="block text-sm">
                Добавить репозиторий
                <Select
                  className="mt-2 w-full"
                  value={selected}
                  onValueChange={(nextValue) => setSelected(nextValue)}
                  options={[
                    { value: '', label: 'Выберите репозиторий' },
                    ...(available.data?.map((repository) => ({
                      value: String(`${repository.forge_instance_id}/${repository.repository_id}`),
                      label: String(repository.public_name),
                    })) ?? []),
                  ]}
                />
              </label>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="outline"
                  disabled={offset === 0}
                  onClick={() => {
                    setOffset(Math.max(0, offset - 50))
                    setSelected('')
                  }}
                >
                  Назад
                </Button>
                <Button
                  variant="outline"
                  disabled={(available.data?.length ?? 0) < 50}
                  onClick={() => {
                    setOffset(offset + 50)
                    setSelected('')
                  }}
                >
                  Далее
                </Button>
                <Button disabled={busy || !selected} onClick={() => void link()}>
                  Связать
                </Button>
              </div>
            </>
          )}
          {error && (
            <p role="alert" className="text-danger">
              {error}
            </p>
          )}
        </div>
      )}
      <div className="space-y-3 border-t border-border pt-3">
        <h3 className="font-medium">PR, коммиты и проверки</h3>
        {evidence.isPending ? (
          <p role="status">Читаем свидетельства…</p>
        ) : evidence.isError ? (
          <p role="alert" className="text-danger">
            Свидетельства Forge недоступны.
          </p>
        ) : evidence.data?.length === 0 ? (
          <p className="text-sm text-text-muted">
            Связанных PR пока нет. Откройте репозиторий из задачи и свяжите PR.
          </p>
        ) : (
          evidence.data?.map((item) => (
            <article
              key={item.pull_request_id}
              className="space-y-2 rounded-md border border-border p-3"
            >
              <p>
                {forge ? (
                  <a
                    className="text-accent"
                    href={withNamespaceLocation(
                      `${forge}/catalog/repositories/${item.repository.repository_id}/pulls/${item.number}`,
                      ref,
                      search,
                    )}
                  >
                    #{item.number} {item.title}
                  </a>
                ) : (
                  item.title
                )}{' '}
                · {item.status}
              </p>
              <p className="break-all text-xs text-text-muted">
                Коммиты: {item.commits.join(', ')}
              </p>
              <ul className="space-y-1 text-sm">
                {item.checks.map((check) => (
                  <li key={check.pipeline_id}>
                    {forge ? (
                      <a
                        className="text-accent"
                        href={withNamespaceLocation(
                          `${forge}/pipelines/${check.pipeline_id}`,
                          ref,
                          search,
                        )}
                      >
                        CI · {check.status}
                      </a>
                    ) : (
                      check.status
                    )}
                    <span className="ml-2 font-mono text-xs">{check.commit_sha.slice(0, 12)}</span>
                  </li>
                ))}
                {item.checks.length === 0 && (
                  <li className="text-text-muted">Проверок этих коммитов пока нет.</li>
                )}
              </ul>
            </article>
          ))
        )}
        <div className="flex gap-2">
          <Button
            variant="outline"
            disabled={evidenceOffset === 0}
            onClick={() => setEvidenceOffset(Math.max(0, evidenceOffset - 10))}
          >
            Назад
          </Button>
          <Button
            variant="outline"
            disabled={(evidence.data?.length ?? 0) < 10}
            onClick={() => setEvidenceOffset(evidenceOffset + 10)}
          >
            Далее
          </Button>
        </div>
      </div>
    </section>
  )
}
