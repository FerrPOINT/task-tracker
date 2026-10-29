import { useScopedMutation as useMutation } from '@/shared/api/useScopedMutation'
import { visibleProjectKeys } from '@/api/project-catalog'
import { useEffect, useId, useRef, useState } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { useQueryClient } from '@tanstack/react-query'
import { Link, useNavigate } from 'react-router'
import {
  Fieldset,
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogHeader,
  AlertDialogTitle,
  Button,
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  ErrorState,
  Input,
  LoadingState,
  Textarea,
} from '@pdlc/ui/ui'
import { useSessionCommand } from '@pdlc/ui/lib'
import { getProject, type Project } from '@/api/project'
import {
  createProjectInSpaces,
  createProjectInSpacesSchema,
  type CreateProjectInSpaces,
} from '@/api/project-spaces'
import { nonNilUuid, SpaceRequestError } from '@/api/space-catalog'
import {
  isCurrentSpaceActor,
  useSpaceSession,
  type SpaceSession,
} from '@/shared/auth/space-session'
import { projectKeys } from '@/shared/api/hooks'
import { SpaceSetField } from './SpaceSetField'
import { ProjectCommandRecovery } from './ProjectCommandRecovery'

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  onCreated?: (project: Project) => void
  navigateOnSuccess?: boolean
}
export function CreateProjectInSpacesDialog(props: Props) {
  const session = useSpaceSession((state) => state.session)
  if (!props.open) return null
  return (
    <Dialog open onOpenChange={props.onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Создать проект</DialogTitle>
        </DialogHeader>
        {!session ? (
          <ErrorState message="Для создания нужна подтверждённая центральная сессия." />
        ) : (
          <ProjectCommandRecovery
            key={JSON.stringify([
              session.identity,
              session.userId,
              session.policyEpoch,
              session.contextEpoch,
            ])}
          >
            <CreateForm {...props} session={session} />
          </ProjectCommandRecovery>
        )}
      </DialogContent>
    </Dialog>
  )
}
function CreateForm({
  session,
  onOpenChange,
  onCreated,
  navigateOnSuccess = true,
}: Props & { session: SpaceSession }) {
  const id = useId()
  const alive = useRef(true)
  useEffect(() => {
    alive.current = true
    return () => {
      alive.current = false
    }
  }, [])
  const cache = useQueryClient()
  const navigate = useNavigate()
  const [catalogReady, setCatalogReady] = useState(false)
  const [discarding, setDiscarding] = useState(false)
  const journal = useSessionCommand<CreateProjectInSpaces>(
    'tracker:create-project-v2:' +
      JSON.stringify([session.identity.issuer, session.identity.subject]),
  )
  const restored = journal.command ? createProjectInSpacesSchema.safeParse(journal.command) : null
  const original = restored?.success ? restored.data : null
  const form = useForm<CreateProjectInSpaces>({
    resolver: zodResolver(createProjectInSpacesSchema),
    mode: 'onChange',
    defaultValues: { key: '', name: '', description: null, spaces: [] },
  })
  function finish(project: Project) {
    journal.clear()
    if (!alive.current || !isCurrentSpaceActor(session)) return
    void cache.invalidateQueries({ queryKey: projectKeys.all })
    void cache.invalidateQueries({ queryKey: visibleProjectKeys.identity(session.identity) })
    onOpenChange(false)
    onCreated?.(project)
    if (navigateOnSuccess) navigate('/projects/' + encodeURIComponent(project.key) + '/board')
  }
  const create = useMutation({
    retry: false,
    mutationFn: (request: CreateProjectInSpaces) => {
      if (!isCurrentSpaceActor(session)) throw new SpaceRequestError(403)
      return createProjectInSpaces(request)
    },
    onSuccess: finish,
  })
  const lookup = useMutation({
    retry: false,
    mutationFn: async (request: CreateProjectInSpaces) => {
      if (!isCurrentSpaceActor(session)) throw new SpaceRequestError(403)
      const project = await getProject(request.key)
      if (project.key !== request.key || !nonNilUuid.safeParse(project.id).success)
        throw new SpaceRequestError(502, 'invalid_response')
      return project
    },
  })
  if (restored && !restored.success)
    return (
      <ErrorState message="Журнал создания повреждён. Запись сохранена, отправка заблокирована." />
    )
  if (session.policyRevision === null)
    return (
      <div className="space-y-3">
        <LoadingState message="Подтверждаем допуск к пространствам…" />
        <SpaceSetField value={original?.spaces ?? []} disabled onChange={() => {}} />
      </div>
    )
  const busy = create.isPending || lookup.isPending
  return (
    <div className="space-y-4">
      {original ? (
        <div className="space-y-3">
          <p role="status">
            Сохранён исходный запрос: {original.key} · {original.name}. Исход ещё не подтверждён.
          </p>
          <p className="text-sm text-text-muted">
            Повтор использует тот же уникальный ключ проекта. Проверка найденного проекта сама по
            себе не подтверждает авторство исходного создания.
          </p>
          <div className="flex flex-wrap gap-2">
            <Button type="button" disabled={busy} onClick={() => lookup.mutate(original)}>
              Проверить проект по ключу
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={busy || !catalogReady}
              onClick={() => create.mutate(original)}
            >
              Повторить исходный запрос
            </Button>
          </div>
          {lookup.isError && (
            <ErrorState message="Найденный проект не подтверждён. Отсутствие ответа или 404 не отменяет исходный запрос; журнал сохранён." />
          )}
          {lookup.data && (
            <div className="space-y-2">
              <p>
                Найден проект: {lookup.data.key} · {lookup.data.name}.
              </p>
              <Link
                className="text-accent underline"
                to={'/projects/' + encodeURIComponent(lookup.data.key) + '/board'}
              >
                Открыть найденный проект
              </Link>
              <Button
                type="button"
                variant="outline"
                disabled={busy}
                onClick={() => {
                  if (isCurrentSpaceActor(session) && lookup.data) finish(lookup.data)
                }}
              >
                Использовать найденный проект
              </Button>
            </div>
          )}
          <SpaceSetField
            value={original.spaces}
            disabled
            onChange={() => {}}
            onAvailabilityChange={setCatalogReady}
          />
          <Button type="button" variant="ghost" disabled={busy} onClick={() => setDiscarding(true)}>
            Отказаться от повторов
          </Button>
        </div>
      ) : (
        <form
          className="space-y-4"
          onSubmit={form.handleSubmit((values) => {
            if (!catalogReady || busy) return
            const request = createProjectInSpacesSchema.parse({
              ...values,
              name: values.name.trim(),
              description: values.description?.trim() || null,
            })
            lookup.reset()
            create.mutate(journal.capture(request))
          })}
        >
          <Fieldset disabled={!catalogReady || busy} className="min-w-0 space-y-3">
            <label className="block space-y-1" htmlFor={id + '-key'}>
              <span>Ключ проекта</span>
              <Input id={id + '-key'} maxLength={10} autoComplete="off" {...form.register('key')} />
            </label>
            {form.formState.errors.key && (
              <p role="alert">Ключ: от 1 до 10 латинских букв, цифр или дефисов.</p>
            )}
            <label className="block space-y-1" htmlFor={id + '-name'}>
              <span>Название проекта</span>
              <Input id={id + '-name'} {...form.register('name')} />
            </label>
            {form.formState.errors.name && <p role="alert">Укажите название до 200 символов.</p>}
            <label className="block space-y-1" htmlFor={id + '-description'}>
              <span>Описание проекта</span>
              <Textarea id={id + '-description'} {...form.register('description')} />
            </label>
          </Fieldset>
          <Controller
            name="spaces"
            control={form.control}
            render={({ field }) => (
              <SpaceSetField
                value={field.value}
                onChange={field.onChange}
                disabled={busy}
                onAvailabilityChange={setCatalogReady}
              />
            )}
          />
          {form.formState.errors.spaces && (
            <p role="alert">Выберите хотя бы одно активное пространство.</p>
          )}
          <Button type="submit" disabled={!catalogReady || busy || !form.formState.isValid}>
            Создать проект
          </Button>
        </form>
      )}
      {create.isError && (
        <ErrorState message="Создание не подтверждено. Исходный запрос сохранён; новый ключ автоматически не создаётся." />
      )}
      <Button type="button" variant="secondary" disabled={busy} onClick={() => onOpenChange(false)}>
        Закрыть
      </Button>
      <AlertDialog open={discarding} onOpenChange={setDiscarding}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Прекратить повторы запроса?</AlertDialogTitle>
            <AlertDialogDescription>
              Это удалит только сохранённый запрос из этой сессии. Уже начавшееся создание не
              отменится: проект с ключом {original?.key} может существовать или появиться позже.
              Перед созданием другого проекта проверьте этот ключ.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="flex justify-end gap-2">
            <AlertDialogCancel>Продолжить проверку</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                journal.clear()
                form.reset()
                create.reset()
                lookup.reset()
                setDiscarding(false)
              }}
            >
              Прекратить повторы
            </AlertDialogAction>
          </div>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
