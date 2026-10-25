import {
  EmptyState,
  TextLink,
  Select,
  ResourceList,
  ResourceListRow,
  LoadingState,
  Button,
  usePlatformServices,
  Card,
  CardContent,
  SectionHeader,
  Notice,
  FormField,
  PageNavigation,
} from '@pdlc/ui/ui'
import { useId, useState } from 'react'
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
  const repositoryField = useId()
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
    <Card asChild>
      <section>
        <CardContent className="space-y-3">
          <SectionHeader title="Репозитории проекта" />
          {fleet && (
            <p>
              <TextLink
                href={withNamespaceLocation(
                  `${fleet}/chats?task_id=${taskId}&tracker_instance_id=${context.data!.binding.resource.instance_id}`,
                  ref,
                )}
              >
                Чаты Fleet с контекстом задачи
              </TextLink>
            </p>
          )}
          {linked.isPending ? (
            <LoadingState message="Загружаем связи…" />
          ) : linked.isError ? (
            <Notice role="alert" tone="danger" appearance="inline">
              Связи репозиториев недоступны.
            </Notice>
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
                        <TextLink
                          href={withNamespaceLocation(
                            `${forge}/catalog/repositories/${repository.repository_id}?task_id=${taskId}&tracker_instance_id=${context.data!.binding.resource.instance_id}`,
                            ref,
                            search,
                          )}
                        >
                          {repository.public_name}
                        </TextLink>
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
            <PageNavigation
              label="Страницы связей проекта"
              page={linkedOffset / 100 + 1}
              hasNext={!linked.isError && (linked.data?.length ?? 0) >= 100}
              pending={linked.isFetching}
              previousLabel="Предыдущие связи"
              nextLabel="Следующие связи"
              onPageChange={(page) => setLinkedPage({ scope, offset: (page - 1) * 100 })}
            />
          )}
          {context.data?.binding.state === 'active' && (
            <div className="space-y-3">
              {available.isError ? (
                <Notice role="alert" tone="danger" appearance="inline">
                  Каталог Forge недоступен. Сохранённые связи остаются выше.
                </Notice>
              ) : (
                <>
                  <FormField id={repositoryField} label="Добавить репозиторий">
                    {(attrs) => (
                      <Select
                        {...attrs}
                        className="w-full"
                        value={selected}
                        onValueChange={(nextValue) => setSelected(nextValue)}
                        options={[
                          { value: '', label: 'Выберите репозиторий' },
                          ...(available.data?.map((repository) => ({
                            value: String(
                              `${repository.forge_instance_id}/${repository.repository_id}`,
                            ),
                            label: String(repository.public_name),
                          })) ?? []),
                        ]}
                      />
                    )}
                  </FormField>
                  <div className="flex flex-wrap gap-2">
                    <PageNavigation
                      label="Страницы каталога Forge"
                      page={offset / 50 + 1}
                      hasNext={(available.data?.length ?? 0) >= 50}
                      previousLabel="Назад"
                      nextLabel="Далее"
                      onPageChange={(page) => {
                        setOffset((page - 1) * 50)
                        setSelected('')
                      }}
                    />
                    <Button disabled={busy || !selected} onClick={() => void link()}>
                      Связать
                    </Button>
                  </div>
                </>
              )}
              {error && (
                <Notice role="alert" tone="danger" appearance="inline">
                  {error}
                </Notice>
              )}
            </div>
          )}
          <div className="space-y-3 border-t border-border pt-3">
            <SectionHeader level={3} title="PR, коммиты и проверки" />
            {evidence.isPending ? (
              <LoadingState message="Читаем свидетельства…" />
            ) : evidence.isError ? (
              <Notice role="alert" tone="danger" appearance="inline">
                Свидетельства Forge недоступны.
              </Notice>
            ) : evidence.data?.length === 0 ? (
              <EmptyState message={'Связанных PR пока нет. Откройте репозиторий из задачи и свяжите PR.'} />
            ) : (
              evidence.data?.map((item) => (
                <Card asChild key={item.pull_request_id}>
                  <article>
                    <CardContent className="space-y-2">
                      <p>
                        {forge ? (
                          <TextLink
                            href={withNamespaceLocation(
                              `${forge}/catalog/repositories/${item.repository.repository_id}/pulls/${item.number}`,
                              ref,
                              search,
                            )}
                          >
                            #{item.number} {item.title}
                          </TextLink>
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
                              <TextLink
                                href={withNamespaceLocation(
                                  `${forge}/pipelines/${check.pipeline_id}`,
                                  ref,
                                  search,
                                )}
                              >
                                CI · {check.status}
                              </TextLink>
                            ) : (
                              check.status
                            )}
                            <span className="ml-2 font-mono text-xs">
                              {check.commit_sha.slice(0, 12)}
                            </span>
                          </li>
                        ))}
                        {item.checks.length === 0 && (
                          <li className="text-text-muted">Проверок этих коммитов пока нет.</li>
                        )}
                      </ul>
                    </CardContent>
                  </article>
                </Card>
              ))
            )}
            <PageNavigation
              label="Страницы свидетельств Forge"
              page={evidenceOffset / 10 + 1}
              hasNext={(evidence.data?.length ?? 0) >= 10}
              previousLabel="Назад"
              nextLabel="Далее"
              onPageChange={(page) => setEvidenceOffset((page - 1) * 10)}
            />
          </div>
        </CardContent>
      </section>
    </Card>
  )
}
