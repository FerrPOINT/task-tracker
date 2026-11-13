import { useEffect, useRef, useState, type FormEvent } from 'react'
import { useBlocker, useLocation, useNavigate } from 'react-router'
import { useInfiniteQuery, useQuery, useQueryClient } from '@tanstack/react-query'
import { useScopedMutation } from '@/shared/api/useScopedMutation'
import { useAuthStore } from '@/shared/auth/store'
import { capturePlatformPrivateState } from '@pdlc/ui/platform'
import {
  ActionGroup,
  Button,
  Notice,
  LoadingState,
  ErrorState,
  EmptyState,
  ConfirmDialog,
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  FormField,
  Select,
  Disclosure,
} from '@pdlc/ui/ui'
import {
  readPlan,
  savePlan,
  readHistory,
  readRevision,
  readOperation,
  PlanRequestError,
  type PlanContent,
  type PlanRevision,
  type SavePlan,
} from '@/api/decomposition'
import { PlanForm } from './form'
import { PlanRead } from './plan'

/** Task and identity are remount boundaries; server state belongs to React Query. */
export function DecompositionPanel({ issueId }: { issueId: string }) {
  const user = useAuthStore((s) => s.userId)
  const authVersion = useAuthStore((s) => s.authVersion ?? 0)
  return <PlanPanel key={`${issueId}:${user}:${authVersion}`} issueId={issueId} />
}
function PlanPanel({ issueId }: { issueId: string }) {
  const navigate = useNavigate()
  const location = useLocation()
  const client = useQueryClient()
  const user = useAuthStore((s) => s.userId)
  const token = useAuthStore((s) => s.token)
  const authVersion = useAuthStore((s) => s.authVersion ?? 0)
  const key = ['decomposition', issueId, user, authVersion] as const
  const query = useQuery({
    queryKey: key,
    queryFn: ({ signal }) => readPlan(issueId, signal),
    retry: false,
  })
  const [draft, setDraft] = useState<PlanContent | null>(null)
  const [baseRevision, setBaseRevision] = useState(0)
  const [reason, setReason] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState('')
  const [denied, setDenied] = useState(false)
  const [conflict, setConflict] = useState<Awaited<ReturnType<typeof readPlan>> | null>(null)
  const [historyOpen, setHistoryOpen] = useState(false)
  const [selectedRevision, setSelectedRevision] = useState(0)
  const latestInput = useRef({ draft, reason })
  latestInput.current = { draft, reason }
  const uncertain = useRef<SavePlan | null>(null)
  const mounted = useRef(true)
  const editRef = useRef<HTMLButtonElement>(null)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  const mutation = useScopedMutation({
    mutationFn: (command: SavePlan) => savePlan(issueId, command),
    retry: false,
  })
  const plan = query.data?.plan
  const restriction = query.data?.edit_restriction
  const blocker = useBlocker(
    ({ currentLocation, nextLocation }) =>
      Boolean(draft) && currentLocation.pathname !== nextLocation.pathname,
  )
  useEffect(() => {
    if (!draft) return
    const leave = (event: BeforeUnloadEvent) => {
      event.preventDefault()
      event.returnValue = ''
    }
    window.addEventListener('beforeunload', leave)
    return () => window.removeEventListener('beforeunload', leave)
  }, [draft])
  const history = useInfiniteQuery({
    queryKey: [...key, 'history'],
    initialPageParam: undefined as number | undefined,
    queryFn: ({ pageParam, signal }) => readHistory(issueId, pageParam, signal),
    getNextPageParam: (page) => page.next_before ?? undefined,
    enabled: historyOpen && !denied,
    retry: false,
  })
  const snapshot = useQuery({
    queryKey: [...key, 'revision', selectedRevision],
    queryFn: ({ signal }) => readRevision(issueId, selectedRevision, signal),
    enabled: historyOpen && selectedRevision > 0 && !denied,
    retry: false,
  })
  function beginEdit() {
    setDraft(
      structuredClone(
        plan?.content ?? { overview: '', source: query.data?.current_source ?? null, sections: [] },
      ),
    )
    setBaseRevision(plan?.revision ?? 0)
    setReason('')
    setError(null)
    setNotice('')
    setConflict(null)
    queueMicrotask(() => document.getElementById('plan-overview')?.focus())
  }
  function cancelEdit() {
    setDraft(null)
    setConflict(null)
    setError(null)
    queueMicrotask(() => editRef.current?.focus())
  }
  function denyAccess() {
    setDenied(true)
    setDraft(null)
    setConflict(null)
    uncertain.current = null
    client.removeQueries({ queryKey: key })
  }
  async function submit(event: FormEvent) {
    event.preventDefault()
    if (!draft || busy || conflict || denied) return
    const ownerToken = token
    const stillPrivate = capturePlatformPrivateState()
    const current = () =>
      mounted.current && ownerToken === useAuthStore.getState().token && stillPrivate()
    const input = JSON.stringify({ draft, reason })
    const prior = uncertain.current
    const command: SavePlan = prior ?? {
      content: structuredClone(draft),
      expected_revision: baseRevision,
      reason,
      idempotency_key: crypto.randomUUID(),
      fence: null,
    }
    setError(null)
    setBusy(true)
    try {
      // An uncertain request keeps its original key/payload even if the draft changes.
      // A 404 readback permits the exact idempotent POST, never a different mutation.
      let saved
      if (prior) {
        try {
          saved = await readOperation(issueId, prior.idempotency_key)
        } catch (cause) {
          if (!(cause instanceof PlanRequestError) || cause.status !== 404) throw cause
        }
      }
      if (!current()) return
      if (!saved) saved = await mutation.mutateAsync(command)
      if (!current()) return
      uncertain.current = null
      const sameInput =
        input === JSON.stringify(latestInput.current) &&
        (!prior ||
          (JSON.stringify(prior.content) === JSON.stringify(draft) && prior.reason === reason))
      if (sameInput) {
        setDraft(null)
        setReason('')
      } else setBaseRevision(saved.result.revision)
      setNotice(
        `${saved.changed ? `Сохранена редакция ${saved.result.revision}.` : `План не изменился. Редакция ${saved.result.revision}.`}${sameInput ? '' : ' Новый черновик остался в форме; проверьте его и сохраните отдельно.'}`,
      )
      await client.invalidateQueries({ queryKey: key })
      if (current() && sameInput) queueMicrotask(() => editRef.current?.focus())
    } catch (cause) {
      if (!current()) return
      if (cause instanceof PlanRequestError && [401, 403, 404].includes(cause.status)) {
        denyAccess()
        return
      }
      if (cause instanceof PlanRequestError && cause.status === 409) {
        uncertain.current = null
        try {
          const latest = await readPlan(issueId)
          if (current()) setConflict(latest)
        } catch (readError) {
          if (current()) {
            if (readError instanceof PlanRequestError && [401, 403, 404].includes(readError.status))
              denyAccess()
            else
              setError(
                'Не удалось прочитать актуальный план. Черновик сохранён; повторите проверку.',
              )
          }
        }
      } else if (cause instanceof PlanRequestError && cause.status === 422) {
        uncertain.current = null
        setError('Проверьте содержание плана и причину изменения. Ввод сохранён.')
      } else {
        uncertain.current = command
        setError(
          'Результат сохранения пока неизвестен. Ввод сохранён. Повторная отправка сначала проверит исходную операцию с тем же ключом.',
        )
      }
    } finally {
      if (current()) setBusy(false)
    }
  }
  const leaveDialog = (
    <ConfirmDialog
      open={blocker.state === 'blocked'}
      title="Покинуть задачу?"
      description="Несохранённые изменения будут потеряны. Между вкладками черновик сохраняется."
      confirmLabel="Покинуть без сохранения"
      cancelLabel="Остаться"
      onOpenChange={(open) => {
        if (!open && blocker.state === 'blocked') blocker.reset()
      }}
      onConfirm={() => {
        setDraft(null)
        if (blocker.state === 'blocked') blocker.proceed()
      }}
    />
  )
  if (
    denied ||
    [query.error, history.error, snapshot.error].some(
      (cause) => cause instanceof PlanRequestError && [401, 403, 404].includes(cause.status),
    )
  )
    return (
      <>
        <ErrorState scope="local" message="Доступ к плану недоступен. Обновите доступ к задаче." />
        {leaveDialog}
      </>
    )
  if (query.isPending)
    return (
      <>
        <LoadingState scope="local" message="Загружаем декомпозицию…" />
        {leaveDialog}
      </>
    )
  if (query.isError)
    return (
      <>
        <ErrorState
          scope="local"
          message="Не удалось загрузить декомпозицию"
          onRetry={() => void query.refetch()}
        />
        {leaveDialog}
      </>
    )
  const stale = Boolean(
    plan && plan.content.source?.requirements_hash !== query.data.current_source?.requirements_hash,
  )
  return (
    <div className="min-w-0 space-y-4">
      <div className="flex max-w-4xl flex-wrap items-center justify-between gap-x-3 gap-y-1">
        <div className="flex min-w-0 flex-wrap items-center gap-x-1 text-sm">
          {plan ? (
            <>
              <Button
                variant="link"
                size="sm"
                disabled={Boolean(draft)}
                aria-label={`Редакция ${plan.revision} — история изменений плана`}
                onClick={() => {
                  setSelectedRevision(plan.revision)
                  setHistoryOpen(true)
                }}
              >
                Редакция {plan.revision}
              </Button>
              <span className="min-w-0 break-words">
                {' '}
                · {plan.author_name}
                {plan.content.source
                  ? ` · требования ${plan.content.source.requirements_revision}`
                  : ''}
              </span>
            </>
          ) : (
            'План реализации ещё не подготовлен'
          )}
        </div>
        <ActionGroup align="start">
          <Button
            variant="link"
            onClick={() => {
              const params = new URLSearchParams(location.search)
              params.set('tab', 'sdlc')
              navigate({ search: params.toString(), hash: '' })
            }}
          >
            Требования задачи
          </Button>
          <Button
            ref={editRef}
            variant="outline"
            disabled={!query.data.can_edit || Boolean(draft)}
            onClick={beginEdit}
          >
            {plan ? 'Редактировать' : 'Создать план'}
          </Button>
        </ActionGroup>
      </div>
      {restriction && <Notice tone="warning">{restriction}</Notice>}
      {stale && (
        <Notice tone="warning">
          Требования изменились. Пересмотрите план и сохраните редакцию со ссылкой на актуальные
          требования.
        </Notice>
      )}
      {notice && (
        <Notice role="status" tone="success" appearance="inline">
          {notice}
        </Notice>
      )}
      {conflict && (
        <Notice tone="warning" title="План или условия редактирования изменились" role="alert">
          <p>Ваш черновик остался в форме. Сравните его с актуальной редакцией и требованиями.</p>
          {conflict.plan && (
            <Disclosure summary={`Актуальная редакция ${conflict.plan.revision}`}>
              <PlanRead plan={conflict.plan.content} snapshot />
            </Disclosure>
          )}
          {conflict.edit_restriction && <p>{conflict.edit_restriction}</p>}
          <Button
            variant="outline"
            disabled={!conflict.can_edit}
            onClick={() => {
              setBaseRevision(conflict.plan?.revision ?? 0)
              setDraft((value) => value && { ...value, source: conflict.current_source })
              setConflict(null)
              setNotice('Базовая редакция обновлена. Согласуйте черновик и сохраните явно.')
            }}
          >
            Я сравнил изменения — продолжить правку
          </Button>
        </Notice>
      )}
      {draft ? (
        <PlanForm
          draft={draft}
          setDraft={setDraft}
          reason={reason}
          setReason={setReason}
          pending={busy}
          reasonRequired={
            baseRevision > 0 && JSON.stringify(draft) !== JSON.stringify(plan?.content)
          }
          error={error}
          blocked={Boolean(conflict) || !query.data.can_edit}
          savePlan={(event) => void submit(event)}
          cancelEdit={cancelEdit}
        />
      ) : plan ? (
        <PlanRead plan={plan.content} />
      ) : (
        <EmptyState
          message="Декомпозиции пока нет"
          description="План объединяет разделы одной задачи. Для небольшой работы достаточно одного раздела."
        />
      )}
      {leaveDialog}
      <Dialog open={historyOpen} onOpenChange={setHistoryOpen}>
        <DialogContent className="max-w-3xl">
          <DialogHeader>
            <DialogTitle>Редакции плана</DialogTitle>
            <DialogDescription>
              Авторы, причины изменений и предыдущие редакции. Только чтение.
            </DialogDescription>
          </DialogHeader>
          <div className="max-h-[65vh] space-y-4 overflow-y-auto">
            {history.isPending ? (
              <LoadingState scope="local" />
            ) : history.isError ? (
              <ErrorState
                scope="local"
                message="Не удалось загрузить историю"
                onRetry={() => void history.refetch()}
              />
            ) : (
              <>
                <FormField id="plan-history-revision" label="Редакция">
                  {(attrs) => (
                    <Select
                      {...attrs}
                      value={String(selectedRevision)}
                      onValueChange={(v) => setSelectedRevision(Number(v))}
                      options={history.data.pages
                        .flatMap((page) => page.revisions)
                        .map((r) => ({
                          value: String(r.revision),
                          label: `Редакция ${r.revision} · ${r.author_name} · ${r.reason || 'Создание плана'}`,
                        }))}
                    />
                  )}
                </FormField>
                {history.hasNextPage && (
                  <Button
                    variant="outline"
                    disabled={history.isFetchingNextPage}
                    onClick={() => void history.fetchNextPage()}
                  >
                    Более ранние редакции
                  </Button>
                )}
              </>
            )}
            {snapshot.isPending ? (
              <LoadingState scope="local" />
            ) : snapshot.isError ? (
              <ErrorState
                scope="local"
                message="Не удалось прочитать редакцию"
                onRetry={() => void snapshot.refetch()}
              />
            ) : (
              snapshot.data && <RevisionSnapshot plan={snapshot.data} />
            )}
          </div>
        </DialogContent>
      </Dialog>
    </div>
  )
}
function RevisionSnapshot({ plan }: { plan: PlanRevision }) {
  return <PlanRead key={plan.revision} plan={plan.content} snapshot />
}
