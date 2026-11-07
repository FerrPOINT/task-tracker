import { useEffect, useRef, useState, type FormEvent } from 'react'
import { useBlocker, useLocation, useNavigate } from 'react-router'
import {
  Button,
  ActionGroup,
  Prose,
  Notice,
  Disclosure,
  FormField,
  Select,
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  ConfirmDialog,
  EmptyState,
  ErrorState,
  LoadingState,
} from '@pdlc/ui/ui'
import { PlanForm } from '@/features/decomposition/form'
import { PlanRead } from '@/features/decomposition/plan'
import { fixture, remoteRevision, revise, type Plan, type Scenario } from './model'

/** Local design controller, mounted inside the real IssueDetailPage in development only. */
export function DecompositionDesignPanel({ scenario = 'detailed' }: { scenario?: Scenario }) {
  const navigate = useNavigate()
  const location = useLocation()
  const [plan, setPlan] = useState(() => fixture(scenario))
  const [history, setHistory] = useState<Plan[]>(() => {
    const initial = fixture(scenario)
    return initial
      ? [
          {
            ...structuredClone(initial),
            revision: 1,
            reason: 'Первая редакция плана',
            overview:
              scenario === 'single'
                ? initial.overview
                : 'Используем существующий профиль пользователя и форму настроек.',
            sections: structuredClone(initial.sections),
          },
        ]
      : []
  })
  const [draft, setDraft] = useState<Plan | null>(null)
  const [reason, setReason] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState('')
  const [pending, setPending] = useState(false)
  const [failedOnce, setFailedOnce] = useState(false)
  const [conflict, setConflict] = useState<Plan | null>(null)
  const [conflictAcknowledged, setConflictAcknowledged] = useState(false)
  const [resolvedRead, setResolvedRead] = useState(false)
  const [historyOpen, setHistoryOpen] = useState(false)
  const [historyRevision, setHistoryRevision] = useState('1')
  const editRef = useRef<HTMLButtonElement>(null)
  const counter = useRef(0)
  const dirty = Boolean(draft)
  const blockedReason =
    scenario === 'readonly'
      ? 'У вас есть доступ к просмотру. Изменение плана недоступно.'
      : scenario === 'busy'
        ? 'Агент изменяет задачу. План доступен для чтения; редактирование станет доступно после завершения назначения.'
        : scenario === 'stale'
          ? 'Требования обновлены до редакции 4. План относится к редакции 3 и требует пересмотра.'
          : ''
  const blocker = useBlocker(
    ({ currentLocation, nextLocation }) =>
      dirty &&
      (currentLocation.pathname !== nextLocation.pathname ||
        new URLSearchParams(currentLocation.search).get('scenario') !==
          new URLSearchParams(nextLocation.search).get('scenario')),
  )
  useEffect(() => {
    if (!dirty) return
    const leave = (event: BeforeUnloadEvent) => {
      event.preventDefault()
      event.returnValue = ''
    }
    window.addEventListener('beforeunload', leave)
    return () => window.removeEventListener('beforeunload', leave)
  }, [dirty])

  function beginEdit() {
    setDraft(
      structuredClone(
        plan ?? {
          revision: 0,
          requirementsRevision: 3,
          overview: '',
          author: '',
          reason: '',
          sections: [],
        },
      ),
    )
    setReason('')
    setError(null)
    setNotice('')
    queueMicrotask(() => document.getElementById('plan-overview')?.focus())
  }
  function cancelEdit() {
    setDraft(null)
    setConflict(null)
    setConflictAcknowledged(false)
    setError(null)
    queueMicrotask(() => editRef.current?.focus())
  }
  async function canSave(checkPlanConflict = false): Promise<boolean> {
    if (blockedReason || scenario === 'revoked') return false
    setError(null)
    setPending(true)
    await new Promise((resolve) => setTimeout(resolve, 200))
    setPending(false)
    if (scenario === 'save-error' && !failedOnce) {
      setFailedOnce(true)
      setError('Не удалось сохранить. Данные остались в форме; можно повторить.')
      return false
    }
    if (checkPlanConflict && scenario === 'conflict' && !failedOnce && plan) {
      setFailedOnce(true)
      setConflict(remoteRevision(plan))
      return false
    }
    return true
  }
  async function savePlan(event: FormEvent) {
    event.preventDefault()
    if (!draft || (conflict && !conflictAcknowledged) || pending) return
    if (
      !draft.overview.trim() ||
      !draft.sections.length ||
      draft.sections.some(
        (s) =>
          !s.title.trim() || !s.description.trim() || !s.criteria.trim() || !s.requirements.trim(),
      ) ||
      (plan && !reason.trim())
    ) {
      setError('Заполните решение, разделы, критерии, связь с требованиями и причину изменения.')
      return
    }
    if (!(await canSave(true))) return
    const next = revise(plan, draft, reason.trim() || 'Создание плана')
    if (plan) setHistory((rows) => [...rows, structuredClone(plan)])
    setPlan(next)
    setDraft(null)
    setConflict(null)
    setConflictAcknowledged(false)
    setNotice(`Сохранена редакция ${next.revision}.`)
    queueMicrotask(() => editRef.current?.focus())
  }
  return (
    <>
      <div className="min-w-0 space-y-2">
        <Notice appearance="inline">
          Пример: настройки уведомлений. Данные плана демонстрационные, изменения сохраняются только
          в памяти.
        </Notice>
        {scenario === 'revoked' ? (
          <ErrorState scope="local" message="Доступ к задаче отозван. План недоступен." />
        ) : scenario === 'loading' && !resolvedRead ? (
          <>
            <LoadingState scope="local" message="Загружаем декомпозицию…" />
            <Button variant="outline" onClick={() => setResolvedRead(true)}>
              Завершить демонстрационную загрузку
            </Button>
          </>
        ) : scenario === 'error' && !resolvedRead ? (
          <ErrorState
            scope="local"
            message="Не удалось загрузить декомпозицию."
            onRetry={() => setResolvedRead(true)}
          />
        ) : (
          <>
            <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
              <div className="flex min-w-0 flex-wrap items-center gap-x-1 text-sm">
                {plan ? (
                  <>
                    <Button
                      variant="link"
                      size="sm"
                      disabled={Boolean(draft)}
                      aria-label={`Редакция ${plan.revision} — история изменений плана`}
                      title="Предыдущие редакции, авторы и причины изменений"
                      onClick={() => {
                        setHistoryRevision(String(plan.revision))
                        setHistoryOpen(true)
                      }}
                    >
                      Редакция {plan.revision}
                    </Button>
                    <span> · {plan.author}</span>
                  </>
                ) : (
                  'План реализации ещё не подготовлен'
                )}
              </div>
              <ActionGroup align="start">
                {plan && (
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
                )}
                <Button
                  ref={editRef}
                  variant="outline"
                  disabled={Boolean(blockedReason) || Boolean(draft)}
                  onClick={beginEdit}
                >
                  {plan ? 'Редактировать' : 'Создать план'}
                </Button>
              </ActionGroup>
            </div>
            {blockedReason && <Notice tone="warning">{blockedReason}</Notice>}
            {notice && (
              <Notice role="status" tone="success" appearance="inline">
                {notice}
              </Notice>
            )}
            {conflict && (
              <Notice tone="warning" title="План изменился, пока вы редактировали" role="alert">
                <div className="space-y-3">
                  <p>
                    Ваш черновик сохранён ниже. Сравните его с актуальной редакцией{' '}
                    {conflict.revision}. Продолжение не сохраняет данные автоматически.
                  </p>
                  <Disclosure
                    summary={`Актуальная редакция ${conflict.revision} · ${conflict.author}`}
                    open
                  >
                    <Prose preserveLineBreaks>{conflict.overview}</Prose>
                  </Disclosure>
                  <Button
                    variant="outline"
                    disabled={conflictAcknowledged}
                    onClick={() => {
                      setHistory((rows) => (plan ? [...rows, structuredClone(plan)] : rows))
                      setPlan(conflict)
                      setConflictAcknowledged(true)
                      setDraft((value) => value && { ...value, revision: conflict.revision })
                      setNotice(
                        'Базовая редакция обновлена. Согласуйте свой черновик с изменениями выше и сохраните явно.',
                      )
                    }}
                  >
                    {conflictAcknowledged
                      ? 'Сравнение подтверждено — можно править и сохранять'
                      : 'Я сравнил изменения — продолжить правку'}
                  </Button>
                </div>
              </Notice>
            )}
            {draft ? (
              <PlanForm
                draft={draft}
                setDraft={setDraft}
                reason={reason}
                setReason={setReason}
                pending={pending}
                error={error}
                reasonRequired={Boolean(plan)}
                blocked={Boolean(conflict && !conflictAcknowledged)}
                savePlan={savePlan}
                cancelEdit={cancelEdit}
                createId={() => `new-${++counter.current}`}
              />
            ) : plan ? (
              <PlanRead plan={plan} />
            ) : (
              <EmptyState
                message="Декомпозиции пока нет"
                description="Архитектор подготовит план в этой задаче. Для небольшой работы достаточно одного раздела."
              />
            )}
          </>
        )}
      </div>
      <ConfirmDialog
        open={blocker.state === 'blocked'}
        title="Покинуть задачу?"
        description="Несохранённые изменения будут потеряны. Между вкладками черновик сохраняется."
        onOpenChange={(open) => {
          if (!open && blocker.state === 'blocked') blocker.reset()
        }}
        confirmLabel="Покинуть без сохранения"
        cancelLabel="Остаться"
        onConfirm={() => {
          setDraft(null)
          if (blocker.state === 'blocked') blocker.proceed()
        }}
      />
      <Dialog open={historyOpen} onOpenChange={setHistoryOpen}>
        <DialogContent className="max-w-3xl">
          <DialogHeader>
            <DialogTitle>Редакции плана</DialogTitle>
            <DialogDescription>
              Предыдущие версии плана, их авторы и причины изменений. Только чтение.
            </DialogDescription>
          </DialogHeader>
          <div className="max-h-[65vh] space-y-4 overflow-y-auto">
            <FormField id="plan-history-revision" label="Редакция">
              {(attrs) => (
                <Select
                  {...attrs}
                  value={historyRevision}
                  onValueChange={setHistoryRevision}
                  options={[...history, ...(plan ? [plan] : [])]
                    .slice()
                    .reverse()
                    .map((item) => ({
                      value: String(item.revision),
                      label: `Редакция ${item.revision} · ${item.author} · ${item.reason}`,
                    }))}
                />
              )}
            </FormField>
            {(() => {
              const selected = [...history, ...(plan ? [plan] : [])].find(
                (item) => String(item.revision) === historyRevision,
              )
              return selected ? <PlanRead key={selected.revision} plan={selected} snapshot /> : null
            })()}
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}
