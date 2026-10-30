import { useLayoutEffect, useState, type ReactNode } from 'react'
import { TextLink, ErrorState, LoadingState, SearchableSelect } from '@pdlc/ui/ui'
import { observeSpaceContext, useSpaceSession } from '@/shared/auth/space-session'
import {
  readSpaceLocation,
  workspaceContextKey,
  spaceProjectLocation,
} from '@/shared/lib/space-location'
import { Link, useLocation, useNavigate, useMatches } from 'react-router'
import { useVisibleProjects } from '@/shared/api/useVisibleProjects'
import { useVisibleSpaces } from '@/shared/api/useVisibleSpaces'
import { spaceKey } from '@/api/space-catalog'

export function SpaceContextBoundary({
  children,
  projectKey,
  independent = false,
}: {
  children: ReactNode
  projectKey?: string
  independent?: boolean
}) {
  const location = useLocation()
  const session = useSpaceSession((state) => state.session)
  const key = workspaceContextKey(location.pathname, location.search)
  const context = readSpaceLocation(location.search)
  const projectKeys = new URLSearchParams(location.search).getAll('project_key')
  const invalidProject =
    projectKeys.length > 1 ||
    (projectKeys.length === 1 &&
      (!/^[A-Za-z0-9-]{1,10}$/.test(projectKeys[0]!) ||
        (projectKey !== undefined && projectKeys[0] !== projectKey)))
  const enabled = import.meta.env.VITE_SPACE_ACCESS_ENABLED === 'true'
  useLayoutEffect(() => {
    if (enabled && session) observeSpaceContext(session.identity, key)
  }, [enabled, session, key])
  const ready = Boolean(session && session.contextKey === key)
  const proof = useVisibleProjects(
    { space: context.space, projectKey, limit: 1 },
    enabled && !independent && ready && !context.invalid && !invalidProject,
  )
  if (!enabled || independent) return children
  if (context.invalid || invalidProject)
    return (
      <div className="space-y-3">
        <ErrorState message="Контекст пространства или проекта неоднозначен. Выберите его явно." />
        <TextLink asChild>
          <Link to={spaceProjectLocation(null)}>Открыть все доступные проекты</Link>
        </TextLink>
      </div>
    )
  if (!session) return <ErrorState message="Нужна подтверждённая центральная сессия." />
  if (!ready || (proof.isPending && !proof.unavailable)) return <LoadingState />
  if (!proof.data)
    return (
      <ErrorState
        message="Не удалось подтвердить контекст и доступ к проектам."
        onRetry={() => void proof.refetch()}
      />
    )
  if (
    projectKey &&
    (proof.data.projects.length !== 1 || proof.data.projects[0]?.key !== projectKey)
  )
    return <ErrorState message="Проект недоступен в выбранном пространстве." />
  return children
}
export function SpaceRouteOutlet({
  children,
  projectKey,
}: {
  children: ReactNode
  projectKey?: string
}) {
  const matches = useMatches()
  const independent = matches.some(
    (match) =>
      (match.handle as { spaceAdmission?: string } | undefined)?.spaceAdmission === 'independent',
  )
  return (
    <SpaceContextBoundary projectKey={projectKey} independent={independent}>
      {children}
    </SpaceContextBoundary>
  )
}
const spaceStateLabels = {
  active: 'Активно',
  archived: 'В архиве',
  archiving: 'Архивируется',
  restoring: 'Восстанавливается',
  provisioning: 'Подготавливается',
}
export function SpaceShellContext({ projectKey }: { projectKey?: string }) {
  const location = useLocation(),
    navigate = useNavigate()
  const context = readSpaceLocation(location.search)
  const [spaceQuery, setSpaceQuery] = useState({ q: '', offset: 0, limit: 10 })
  const [projectQuery, setProjectQuery] = useState({ q: '', offset: 0, limit: 10 })
  const spaces = useVisibleSpaces(spaceQuery)
  const projects = useVisibleProjects(
    { ...projectQuery, space: context.space, workingOnly: true },
    !context.invalid,
  )
  const exact = useVisibleProjects(
    { space: context.space, projectKey, limit: 1 },
    !context.invalid && Boolean(projectKey),
  )
  const current = exact.data?.projects.find((project) => project.key === projectKey)
  const projectValue =
    current && exact.data ? exact.data.tracker_instance_id + '/' + current.id : ''
  const spaceValue = context.space ? spaceKey(context.space) : ''
  const session = useSpaceSession((state) => state.session)
  if (!session) return <ErrorState message="Нужна подтверждённая центральная сессия." />
  if (session.policyRevision === null) {
    if (spaces.unavailable && projects.unavailable)
      return (
        <ErrorState
          message="Каталог пространств недоступен."
          onRetry={() => {
            void spaces.refetch()
            void projects.refetch()
          }}
        />
      )
    return <LoadingState message="Подтверждаем контекст пространств…" />
  }
  return (
    <div className="flex min-w-0 flex-wrap gap-2">
      <div className="min-w-0 max-w-64">
        <SearchableSelect
          label="Пространство"
          value={spaceValue}
          selectedLabel={
            spaces.data?.spaces.find((entry) => spaceKey(entry.space) === spaceValue)?.metadata
              .name ?? spaceValue
          }
          placeholder="Все доступные пространства"
          options={(spaces.data?.spaces ?? []).map((entry) => ({
            value: spaceKey(entry.space),
            label:
              entry.metadata.name +
              ' · ' +
              entry.metadata.slug +
              ' · ' +
              spaceStateLabels[entry.state] +
              ' · ' +
              entry.space.space_id,
          }))}
          search={spaceQuery.q}
          onSearch={(q) =>
            setSpaceQuery({ ...spaceQuery, q: Array.from(q).slice(0, 200).join(''), offset: 0 })
          }
          offset={spaceQuery.offset}
          pageSize={10}
          total={spaces.data?.total ?? 0}
          onPage={(offset) => setSpaceQuery({ ...spaceQuery, offset })}
          loading={spaces.isPending}
          error={spaces.unavailable ? 'Не удалось подтвердить пространства' : undefined}
          onRetry={() => void spaces.refetch()}
          aria-invalid={context.invalid || undefined}
          onChange={(value) => {
            const selected = spaces.data?.spaces.find((entry) => spaceKey(entry.space) === value)
            if (!value || selected) navigate(spaceProjectLocation(selected?.space ?? null))
          }}
        />
      </div>
      <div className="min-w-0 max-w-64">
        <SearchableSelect
          label="Проект"
          value={projectValue}
          selectedLabel={current?.name}
          placeholder={
            projectKey && !current ? 'Проверяем выбранный проект' : 'Все проекты пространства'
          }
          options={(projects.data?.projects ?? []).map((project) => ({
            value: projects.data!.tracker_instance_id + '/' + project.id,
            label: project.name + ' · ' + project.key,
          }))}
          search={projectQuery.q}
          onSearch={(q) =>
            setProjectQuery({ ...projectQuery, q: Array.from(q).slice(0, 200).join(''), offset: 0 })
          }
          offset={projectQuery.offset}
          pageSize={10}
          total={projects.data?.total ?? 0}
          onPage={(offset) => setProjectQuery({ ...projectQuery, offset })}
          loading={projects.isPending}
          disabled={context.invalid}
          error={projects.unavailable ? 'Не удалось подтвердить проекты' : undefined}
          onRetry={() => void projects.refetch()}
          onChange={(value) => {
            const selected = projects.data?.projects.find(
              (project) => projects.data!.tracker_instance_id + '/' + project.id === value,
            )
            if (!value || selected) navigate(spaceProjectLocation(context.space, selected?.key))
          }}
        />
      </div>
    </div>
  )
}
