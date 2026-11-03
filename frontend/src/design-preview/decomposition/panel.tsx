import { useEffect, useRef, useState, type FormEvent } from 'react'
import { useBlocker, useLocation, useNavigate } from 'react-router'
import {
  Card,
  CardHeader,
  CardTitle,
  CardContent,
  SectionHeader,
  Button,
  ActionGroup,
  Prose,
  Notice,
  Disclosure,
  FormField,
  Fieldset,
  Input,
  Textarea,
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
import { ArrowUp, ArrowDown, Link as LinkIcon, Plus, Trash2 } from 'lucide-react'
import { fixture, remoteRevision, revise, type Plan, type Section, type Scenario } from './model'

function Field({
  id,
  label,
  value,
  onChange,
  multiline = false,
  required = false,
  hint,
}: {
  id: string
  label: string
  value: string
  onChange: (value: string) => void
  multiline?: boolean
  required?: boolean
  hint?: string
}) {
  return (
    <FormField id={id} label={label} required={required} hint={hint}>
      {(attrs) =>
        multiline ? (
          <Textarea
            {...attrs}
            rows={3}
            value={value}
            onChange={(event) => onChange(event.target.value)}
          />
        ) : (
          <Input {...attrs} value={value} onChange={(event) => onChange(event.target.value)} />
        )
      }
    </FormField>
  )
}

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
  const addSectionRef = useRef<HTMLButtonElement>(null)
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
  function updateSection(id: string, patch: Partial<Section>) {
    setDraft(
      (value) =>
        value && {
          ...value,
          sections: value.sections.map((s) => (s.id === id ? { ...s, ...patch } : s)),
        },
    )
  }
  function move(id: string, direction: number) {
    setDraft((value) => {
      if (!value) return value
      const items = [...value.sections]
      const index = items.findIndex((s) => s.id === id)
      const target = index + direction
      if (index < 0 || target < 0 || target >= items.length) return value
      ;[items[index], items[target]] = [items[target]!, items[index]!]
      return { ...value, sections: items }
    })
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
              <form
                aria-label="Редактирование декомпозиции"
                onSubmit={savePlan}
                className="grid max-w-4xl gap-4"
                aria-busy={pending}
              >
                <Fieldset disabled={pending} className="space-y-4">
                  <Field
                    id="plan-overview"
                    label="Архитектурное решение"
                    multiline
                    required
                    value={draft.overview}
                    onChange={(overview) => setDraft({ ...draft, overview })}
                  />
                  <div className="space-y-3">
                    {draft.sections.map((section, index) => (
                      <Card key={section.id} data-testid={`editor-${section.id}`}>
                        <CardHeader>
                          <CardTitle>Раздел {index + 1}</CardTitle>
                        </CardHeader>
                        <CardContent className="space-y-4">
                          <ActionGroup align="start">
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon"
                              aria-label={`Переместить ${section.title || 'раздел'} вверх`}
                              disabled={index === 0}
                              onClick={() => move(section.id, -1)}
                            >
                              <ArrowUp />
                            </Button>
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon"
                              aria-label={`Переместить ${section.title || 'раздел'} вниз`}
                              disabled={index === draft.sections.length - 1}
                              onClick={() => move(section.id, 1)}
                            >
                              <ArrowDown />
                            </Button>
                            <Button
                              type="button"
                              variant="ghost"
                              onClick={() => {
                                setDraft({
                                  ...draft,
                                  sections: draft.sections.filter((s) => s.id !== section.id),
                                })
                                const neighbor =
                                  draft.sections[index + 1] ?? draft.sections[index - 1]
                                queueMicrotask(() => {
                                  if (neighbor)
                                    document.getElementById(`${neighbor.id}-title`)?.focus()
                                  else addSectionRef.current?.focus()
                                })
                              }}
                            >
                              <Trash2 />
                              Убрать раздел
                            </Button>
                          </ActionGroup>
                          <Field
                            id={`${section.id}-title`}
                            label="Название раздела"
                            required
                            value={section.title}
                            onChange={(title) => updateSection(section.id, { title })}
                          />
                          <Field
                            id={`${section.id}-description`}
                            label="Что сделать"
                            hint="Опишите изменение и ожидаемый результат."
                            multiline
                            required
                            value={section.description}
                            onChange={(description) => updateSection(section.id, { description })}
                          />
                          <Field
                            id={`${section.id}-criteria`}
                            label="Как проверить"
                            multiline
                            required
                            value={section.criteria}
                            onChange={(criteria) => updateSection(section.id, { criteria })}
                          />
                          <Field
                            id={`${section.id}-requirements`}
                            label="Связь с требованиями"
                            required
                            value={section.requirements}
                            onChange={(requirements) => updateSection(section.id, { requirements })}
                          />
                        </CardContent>
                      </Card>
                    ))}
                  </div>
                  <Button
                    ref={addSectionRef}
                    type="button"
                    variant="outline"
                    onClick={() => {
                      counter.current++
                      setDraft({
                        ...draft,
                        sections: [
                          ...draft.sections,
                          {
                            id: `new-${counter.current}`,
                            title: '',
                            description: '',
                            criteria: '',
                            requirements: '',
                          },
                        ],
                      })
                      const newId = `new-${counter.current}`
                      queueMicrotask(() => document.getElementById(`${newId}-title`)?.focus())
                    }}
                  >
                    <Plus />
                    Добавить раздел
                  </Button>
                  <Field
                    id="plan-reason"
                    label="Причина изменения"
                    required={Boolean(plan)}
                    value={reason}
                    onChange={setReason}
                    hint="Сохранится вместе с новой редакцией плана."
                  />
                  <Notice appearance="inline">
                    Содержательно изменённые разделы потребуют повторной проверки. Переименование и
                    порядок сохраняют ссылки.
                  </Notice>
                  {error && (
                    <Notice role="alert" tone="danger">
                      {error}
                    </Notice>
                  )}
                  <ActionGroup separated>
                    <Button
                      type="submit"
                      disabled={pending || Boolean(conflict && !conflictAcknowledged)}
                    >
                      {pending ? 'Сохраняем…' : 'Сохранить план'}
                    </Button>
                    <Button type="button" variant="outline" disabled={pending} onClick={cancelEdit}>
                      Отмена
                    </Button>
                  </ActionGroup>
                </Fieldset>
              </form>
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

function PlanRead({ plan, snapshot = false }: { plan: Plan; snapshot?: boolean }) {
  const location = useLocation()
  const navigate = useNavigate()
  const [open, setOpen] = useState<Set<string>>(() => new Set())
  useEffect(() => {
    const id = location.hash.replace('#section-', '')
    if (snapshot || !id || !plan.sections.some((s) => s.id === id)) return
    setOpen((value) => new Set([...value, id]))
    requestAnimationFrame(() => {
      const element = document.getElementById(`section-${id}`)
      element?.scrollIntoView({ block: 'nearest' })
      element?.querySelector('summary')?.focus()
    })
  }, [location.hash, plan.sections, snapshot])
  return (
    <div className="min-w-0 space-y-2 break-words">
      <Disclosure
        appearance="row"
        summary="Архитектурное решение"
        data-testid="architecture-overview"
      >
        <Prose preserveLineBreaks>{plan.overview}</Prose>
      </Disclosure>
      <div>
        {plan.sections.map((section, index) => (
          <Disclosure
            appearance="row"
            key={section.id}
            id={`${snapshot ? 'snapshot-' : ''}section-${section.id}`}
            open={open.has(section.id)}
            onToggle={(event) => {
              const expanded = event.currentTarget.open
              setOpen((value) => {
                if (value.has(section.id) === expanded) return value
                const next = new Set(value)
                if (expanded) next.add(section.id)
                else next.delete(section.id)
                return next
              })
            }}
            summary={
              <span>
                {index + 1}. {section.title}
              </span>
            }
          >
            <div className="space-y-4 py-2">
              <SectionHeader title="Что сделать" level={4} />
              <Prose>{section.description}</Prose>
              <SectionHeader title="Как проверить" level={4} />
              <Prose>{section.criteria}</Prose>
              <SectionHeader title="Связь с требованиями" level={4} />
              <Prose>{section.requirements}</Prose>
              <ActionGroup align="start">
                {!snapshot && (
                  <Button
                    variant="ghost"
                    onClick={() => {
                      const params = new URLSearchParams(location.search)
                      params.set('tab', 'decomposition')
                      navigate({ search: params.toString(), hash: `section-${section.id}` })
                    }}
                  >
                    <LinkIcon />
                    Ссылка на раздел
                  </Button>
                )}
              </ActionGroup>
            </div>
          </Disclosure>
        ))}
      </div>
    </div>
  )
}
