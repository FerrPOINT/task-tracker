import { useScopedMutation as useMutation } from '@/shared/api/useScopedMutation'
import { visibleProjectKeys } from '@/api/project-catalog'
import { useEffect, useLayoutEffect, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Button, ErrorState, LoadingState } from '@pdlc/ui/ui'
import { useSessionCommand } from '@pdlc/ui/lib'
import type { Project } from '@/api/project'
import {
  getProjectSpaceManagement,
  getProjectSpaceProjection,
  reconcileProjectSpaceProjection,
  getProjectSpaceOperationReceipt,
  replaceProjectSpaces,
  projectSpaceChangeSchema,
  projectSpaceKeys,
  type ReplaceProjectSpaces,
} from '@/api/project-spaces'
import { spaceKey, validityIsCurrent, type SpaceRef, SpaceRequestError } from '@/api/space-catalog'
import {
  isCurrentSpaceActor,
  observeSpacePolicy,
  useSpaceSession,
  type SpaceSession,
} from '@/shared/auth/space-session'
import { SpaceSetField } from './SpaceSetField'
import { ProjectCommandRecovery } from './ProjectCommandRecovery'

export function ProjectSpacesSettings({ project }: { project: Project }) {
  const session = useSpaceSession((state) => state.session)
  if (import.meta.env.VITE_SPACE_ACCESS_ENABLED !== 'true') return null
  if (!session)
    return <ErrorState message="Для пространств требуется подтверждённая центральная сессия." />
  const identityKey = JSON.stringify([
    session.identity,
    session.userId,
    session.policyEpoch,
    session.contextEpoch,
    project.id,
  ])
  return (
    <ProjectCommandRecovery key={identityKey}>
      <Management project={project} session={session} />
    </ProjectCommandRecovery>
  )
}
function sameSet(a: SpaceRef[], b: SpaceRef[]): boolean {
  return (
    a.length === b.length && a.every((ref) => b.some((other) => spaceKey(ref) === spaceKey(other)))
  )
}
function Management({ project, session }: { project: Project; session: SpaceSession }) {
  const cache = useQueryClient()
  const key = [...projectSpaceKeys.project(session.identity, project), 'management']
  const journal = useSessionCommand<ReplaceProjectSpaces>(
    'tracker:project-spaces-v2:' +
      JSON.stringify([session.identity.issuer, session.identity.subject, project.id]),
  )
  const restored = journal.command ? projectSpaceChangeSchema.safeParse(journal.command) : null
  const original = restored?.success ? restored.data : null
  const [draft, setDraft] = useState<{ expectedRevision: number; spaces: SpaceRef[] } | null>(null)
  const [catalogReady, setCatalogReady] = useState(false)
  const [, expire] = useState(0)
  const query = useQuery({
    queryKey: key,
    queryFn: ({ signal }) => getProjectSpaceManagement(project, signal),
    enabled: session.expiresAt > Date.now(),
    retry: false,
    gcTime: 60_000,
    staleTime: 0,
    refetchOnWindowFocus: 'always',
    refetchOnReconnect: 'always',
    refetchIntervalInBackground: true,
    refetchInterval: (value) => {
      if (value.state.status === 'error') return false
      const until = value.state.data?.links.validity.valid_until
      return until ? Math.max(1000, until * 1000 - Date.now()) : false
    },
  })
  const until = query.data?.links.validity.valid_until
  useEffect(() => {
    if (!until) return
    const timer = setTimeout(() => expire((n) => n + 1), Math.max(0, until * 1000 - Date.now()))
    return () => clearTimeout(timer)
  }, [until])
  const current = Boolean(
    !query.isError &&
    query.data &&
    validityIsCurrent(query.data.links.validity) &&
    (session.policyRevision === null || query.data.policyRevision >= session.policyRevision) &&
    session.expiresAt > Date.now(),
  )
  useLayoutEffect(() => {
    if (current && query.data) observeSpacePolicy(session.identity, query.data.policyRevision)
  }, [current, query.data, session.identity])
  const links = current ? query.data?.links : undefined
  const canChange = Boolean(current && query.data?.canChange)
  const value = original?.spaces ?? draft?.spaces ?? links?.spaces ?? []
  const dirty = Boolean(links && !sameSet(value, links.spaces))
  const projectionKey = [...key, 'projection', links?.revision]
  const projection = useQuery({
    queryKey: projectionKey,
    enabled: Boolean(links),
    queryFn: ({ signal }) => getProjectSpaceProjection(project, signal),
    retry: false,
    gcTime: 60_000,
    refetchInterval: (value) =>
      value.state.status === 'error' || value.state.data?.confirmed ? false : 2000,
  })
  const reconcile = useMutation({
    mutationFn: () => {
      if (!isCurrentSpaceActor(session)) throw new SpaceRequestError(403)
      return reconcileProjectSpaceProjection(project)
    },
    onSuccess: (status) => cache.setQueryData(projectionKey, status),
  })
  const receiptRead = useMutation({
    mutationFn: (command: ReplaceProjectSpaces) => {
      if (!isCurrentSpaceActor(session)) throw new SpaceRequestError(403)
      if (!links) throw new SpaceRequestError(503)
      return getProjectSpaceOperationReceipt(project, links.project, command)
    },
    onSuccess: async () => {
      journal.clear()
      setDraft(null)
      await cache.invalidateQueries({ queryKey: key })
      await cache.invalidateQueries({ queryKey: projectionKey })
      await cache.invalidateQueries({ queryKey: visibleProjectKeys.identity(session.identity) })
    },
  })
  const save = useMutation({
    mutationFn: (command: ReplaceProjectSpaces) => {
      if (!isCurrentSpaceActor(session)) throw new SpaceRequestError(403)
      return replaceProjectSpaces(project, command)
    },
    onSuccess: async () => {
      journal.clear()
      setDraft(null)
      // A replay receipt is not a renewed permission. Read the current owner hint.
      await cache.invalidateQueries({ queryKey: key })
      await cache.invalidateQueries({ queryKey: projectionKey })
      await cache.invalidateQueries({ queryKey: visibleProjectKeys.identity(session.identity) })
    },
  })
  if (restored && !restored.success)
    return (
      <ErrorState message="Журнал изменения связей повреждён. Исходная запись сохранена, отправка заблокирована." />
    )
  if (!links)
    return (
      <section className="space-y-3">
        <h2 className="text-lg font-semibold">Пространства проекта</h2>
        {original && <p role="status">Незавершённое изменение сохранено в журнале.</p>}
        {query.isFetching ? (
          <LoadingState />
        ) : (
          <ErrorState
            message="Не удалось подтвердить связи и доступ к проекту."
            onRetry={() => void query.refetch()}
          />
        )}
      </section>
    )
  const projectionConfirmed =
    projection.data?.project.tracker_instance_id === links.project.tracker_instance_id &&
    projection.data.revision === links.revision &&
    projection.data.confirmed
  return (
    <section className="max-w-3xl space-y-4 border-t border-border pt-6">
      <h2 className="text-lg font-semibold">Связи с пространствами</h2>
      <p className="text-sm text-text-muted">
        Ревизия связей: {links.revision}. Названия недоступных пространств не запрашиваются.
      </p>
      {!canChange && (
        <p role="status">
          Связи доступны только для чтения: проверяются роль, активное пространство и архив проекта.
        </p>
      )}
      {(save.isSuccess || receiptRead.isSuccess) && (
        <p role="status">Сохранение подтверждено. Показан текущий набор связей.</p>
      )}
      {save.isError && (
        <ErrorState
          message={
            save.error instanceof SpaceRequestError && save.error.status === 409
              ? 'Набор или состояние проекта изменились. Повтор использует исходный ключ; для нового решения загрузите актуальные связи.'
              : 'Исход изменения не подтверждён. Исходный запрос сохранён для проверки тем же ключом.'
          }
        />
      )}
      {original && (
        <div className="space-y-2 rounded-md border border-border p-3">
          <p className="text-sm">Есть незавершённое изменение. Его набор и ключ сохранены.</p>
          <Button
            type="button"
            disabled={save.isPending || receiptRead.isPending}
            onClick={() => receiptRead.mutate(original)}
          >
            Проверить сохранение
          </Button>
          <Button
            type="button"
            disabled={!canChange || save.isPending || receiptRead.isPending || !catalogReady}
            onClick={() => save.mutate(original)}
          >
            Повторить исходным запросом
          </Button>
          {receiptRead.isError && (
            <p role="alert" className="text-sm text-danger">
              Квитанция не подтверждена. Исход остаётся неизвестным; журнал сохранён.
            </p>
          )}
          {save.error instanceof SpaceRequestError && save.error.status === 409 && (
            <Button
              type="button"
              variant="outline"
              disabled={save.isPending}
              onClick={() => {
                journal.clear()
                setDraft(null)
                save.reset()
                receiptRead.reset()
                void query.refetch()
              }}
            >
              Отменить это изменение и загрузить текущие связи
            </Button>
          )}
        </div>
      )}
      <form
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault()
          if (
            !canChange ||
            !catalogReady ||
            save.isPending ||
            query.isFetching ||
            original ||
            !dirty ||
            !draft
          )
            return
          const command = projectSpaceChangeSchema.parse({
            operation_id: crypto.randomUUID(),
            expected_revision: draft.expectedRevision,
            spaces: draft.spaces,
          })
          receiptRead.reset()
          save.mutate(journal.capture(command))
        }}
      >
        <SpaceSetField
          value={value}
          keepNonEmpty
          disabled={!canChange || save.isPending || Boolean(original) || query.isFetching}
          onAvailabilityChange={setCatalogReady}
          onChange={(spaces) => {
            setDraft({ expectedRevision: draft?.expectedRevision ?? links.revision, spaces })
          }}
        />
        <Button
          type="submit"
          disabled={
            !canChange ||
            !catalogReady ||
            !dirty ||
            save.isPending ||
            Boolean(original) ||
            query.isFetching
          }
        >
          {save.isPending ? 'Сохраняем…' : 'Сохранить связи'}
        </Button>
        {draft && !original && (
          <Button
            type="button"
            variant="outline"
            disabled={save.isPending}
            onClick={() => setDraft(null)}
          >
            Отменить правки
          </Button>
        )}
      </form>
      <div className="space-y-2 rounded-md border border-border p-3">
        <p role="status">
          {projectionConfirmed
            ? 'Wiki подтвердила текущие связи.'
            : projection.isError
              ? 'Не удалось проверить подтверждение Wiki.'
              : 'Ожидаем подтверждение текущих связей в Wiki.'}
        </p>
        {!projectionConfirmed && (
          <Button
            type="button"
            variant="outline"
            disabled={reconcile.isPending || !canChange}
            onClick={() => reconcile.mutate()}
          >
            Проверить синхронизацию
          </Button>
        )}
        {reconcile.isError && (
          <p role="alert" className="text-sm text-danger">
            Синхронизация не подтверждена. Связи и исходная операция сохранены.
          </p>
        )}
      </div>
    </section>
  )
}
