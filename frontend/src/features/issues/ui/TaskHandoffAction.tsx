import { useRef, useState, useId, type RefObject } from 'react'
import { useForm } from 'react-hook-form'
import {
  Button,
  Checkbox,
  Disclosure,
  ErrorState,
  FormDialog,
  FormField,
  LoadingState,
  Notice,
  Textarea,
} from '@pdlc/ui/ui'
import { useScopedMutation } from '@/shared/api/useScopedMutation'
import {
  getHandoffOperation,
  handoffCommandSchema,
  handoffOutcomeUnknown,
  HandoffRequestError,
  type HandoffBlocker,
  type HandoffCommand,
  type HandoffDocument,
} from '@/api/task-handoff'
import { useHandoffPreview, useRequestHandoff, useCancelHandoff } from '../model/task-handoff'
import { stageLabels, lifecycleStatusLabels } from '@/shared/lib/task-lifecycle-labels'

const fieldLabels = {
  scope: 'Рамки работы',
  scenarios: 'Сценарии',
  acceptance_criteria: 'Критерии приёмки',
  checklist: 'Проверки',
  prerequisites: 'Предусловия',
  exclusions: 'За рамками',
  constraints: 'Ограничения',
  dependencies: 'Зависимости',
  assumptions: 'Допущения',
} as const
const mainFields = [
  'scope',
  'scenarios',
  'acceptance_criteria',
  'checklist',
  'prerequisites',
] as const
const extraFields = ['exclusions', 'constraints', 'dependencies', 'assumptions'] as const
type ListField = keyof typeof fieldLabels
type Values = { goal: string } & Record<ListField, string>
function emptyValues(summary: string): Values {
  return {
    goal: summary,
    scope: '',
    scenarios: '',
    acceptance_criteria: '',
    checklist: '',
    prerequisites: '',
    exclusions: '',
    constraints: '',
    dependencies: '',
    assumptions: '',
  }
}
function document(values: Values): HandoffDocument {
  const split = (name: ListField) =>
    values[name]
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
  return {
    goal: values.goal.trim(),
    scope: split('scope'),
    scenarios: split('scenarios'),
    acceptance_criteria: split('acceptance_criteria'),
    checklist: split('checklist'),
    prerequisites: split('prerequisites'),
    exclusions: split('exclusions'),
    constraints: split('constraints'),
    dependencies: split('dependencies'),
    assumptions: split('assumptions'),
  }
}
const blockerLabels: Record<HandoffBlocker, string> = {
  model_not_active: 'Модель задач обновляется.',
  execution_not_configured:
    'Исполнение агентами для проекта не настроено. Задачу можно вести вручную.',
  project_read_only: 'Проект доступен только для чтения.',
  routing_not_configured: 'Для проекта не выбраны исполнители.',
  required_input_missing: 'Не хватает материалов для текущего этапа.',
  application_access_denied:
    'Для запуска нужны разрешённые приложения. Обратитесь к администратору.',
  execution_busy: 'У задачи уже есть запрос на исполнение или незавершённое выполнение.',
  owner_consent_required: 'Передачу должен подтвердить владелец задачи.',
  configuration_changed: 'Настройки исполнителя изменились. Повторите проверку.',
  source_unavailable: 'Исходники проекта не подтверждены или недоступны.',
  workflow_unavailable: 'Процесс исполнения недоступен.',
  fleet_unavailable: 'Сервис исполнителей недоступен.',
  already_completed:
    'Сначала откройте завершённую задачу повторно или перейдите к следующему этапу.',
}
function message(error: unknown) {
  return error instanceof HandoffRequestError
    ? error.message
    : 'Подтверждение не получено. Проверьте результат исходного запроса.'
}

export function TaskHandoffAction({
  issueId,
  summary = '',
  disabled = false,
}: {
  issueId: string
  summary?: string
  disabled?: boolean
}) {
  const [open, setOpen] = useState(false)
  const trigger = useRef<HTMLButtonElement>(null)
  const { query, scope } = useHandoffPreview(issueId, open)
  return (
    <>
      <Button ref={trigger} type="button" size="sm" variant="outline" onClick={() => setOpen(true)}>
        Работа с агентом
      </Button>
      <HandoffForm
        key={`${issueId}:${scope.key}`}
        issueId={issueId}
        summary={summary}
        disabled={disabled}
        open={open}
        setOpen={setOpen}
        query={query}
        authenticated={scope.authenticated}
        trigger={trigger}
      />
    </>
  )
}
function HandoffForm({
  issueId,
  summary,
  disabled,
  open,
  setOpen,
  query,
  authenticated,
  trigger,
}: {
  issueId: string
  summary: string
  disabled: boolean
  open: boolean
  setOpen: (value: boolean) => void
  query: ReturnType<typeof useHandoffPreview>['query']
  authenticated: boolean
  trigger: RefObject<HTMLButtonElement | null>
}) {
  const id = useId()
  const { watch, setValue, reset } = useForm<Values>({ defaultValues: emptyValues(summary) })
  const values = watch()
  const [acknowledged, setAcknowledged] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const request = useRequestHandoff()
  const cancel = useCancelHandoff()
  const attempt = useRef<{ fingerprint: string; command: HandoffCommand } | null>(null)
  const cancellation = useRef<{
    key: string
    handoff: string
    revision: number
    operationKey: string
  } | null>(null)
  const lookup = useScopedMutation({
    mutationFn: ({ key }: { key: string }) => getHandoffOperation(issueId, key),
    retry: false,
    onSuccess: () => {
      void query.refetch()
    },
  })
  const view = query.data
  const current = view?.current_request
  const inputs = document(values)
  const pin = JSON.stringify([
    view?.state,
    view?.issue_sha256,
    view?.routing_policy_version,
    inputs,
  ])
  const foundOriginal = Boolean(
    attempt.current &&
    (current?.operation.receipt.idempotency_key === attempt.current.command.idempotency_key ||
      (lookup.data && lookup.variables?.key === attempt.current.command.idempotency_key)),
  )
  const uncertain = request.isError && handoffOutcomeUnknown(request.error) && !foundOriginal
  const busy = request.isPending || cancel.isPending || lookup.isPending
  const ready = Boolean(
    authenticated &&
    view &&
    !view.blockers.length &&
    view.issue_sha256 &&
    view.routing_policy_version &&
    !current &&
    !query.error &&
    !query.isFetching &&
    !disabled,
  )
  const canSubmit = ready && acknowledged === pin && Boolean(inputs.goal) && !busy && !uncertain
  const dirty =
    !current &&
    (values.goal !== summary ||
      Object.keys(fieldLabels).some((name) => Boolean(values[name as ListField])) ||
      acknowledged !== null)
  async function submit() {
    if (!canSubmit || !view?.issue_sha256 || !view.routing_policy_version) return
    const raw = {
      expected_revision: view.state.revision,
      expected_routing_policy_version: view.routing_policy_version,
      confirmed_issue_sha256: view.issue_sha256,
      requirements: inputs,
    }
    const fingerprint = JSON.stringify(raw)
    const parsed = handoffCommandSchema.safeParse({
      ...raw,
      idempotency_key:
        attempt.current?.fingerprint === fingerprint
          ? attempt.current.command.idempotency_key
          : crypto.randomUUID(),
    })
    if (!parsed.success) {
      setError('Проверьте цель и материалы задачи.')
      return
    }
    attempt.current = { fingerprint, command: parsed.data }
    setError(null)
    try {
      await request.mutateAsync({ id: issueId, command: parsed.data })
      setAcknowledged(null)
    } catch (failure) {
      setError(message(failure))
    }
  }
  async function repeatOriginal() {
    if (!attempt.current || busy || disabled || !authenticated) return
    setError(null)
    try {
      await request.mutateAsync({ id: issueId, command: attempt.current.command })
    } catch (failure) {
      setError(message(failure))
    }
  }
  async function cancelCurrent() {
    if (!current?.can_cancel || busy || disabled) return
    if (
      cancellation.current?.handoff !== current.operation.receipt.handoff_id ||
      cancellation.current.revision !== current.core_revision
    )
      cancellation.current = {
        handoff: current.operation.receipt.handoff_id,
        revision: current.core_revision,
        key: crypto.randomUUID(),
        operationKey: current.operation.receipt.idempotency_key,
      }
    const saved = cancellation.current
    setError(null)
    try {
      await cancel.mutateAsync({
        id: issueId,
        handoff: saved.handoff,
        command: { expected_revision: saved.revision, idempotency_key: saved.key },
      })
    } catch (failure) {
      setError(message(failure))
    }
  }
  const checkKey = uncertain
    ? attempt.current?.command.idempotency_key
    : cancel.isError
      ? cancellation.current?.operationKey
      : undefined
  const renderField = (name: ListField) => (
    <FormField key={name} id={`${id}-${name}`} label={fieldLabels[name]}>
      {(attrs) => (
        <Textarea
          {...attrs}
          rows={2}
          value={values[name]}
          onChange={(event) => setValue(name, event.target.value, { shouldDirty: true })}
          disabled={!ready || uncertain}
        />
      )}
    </FormField>
  )
  return (
    <FormDialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (!next && !uncertain) {
          reset(emptyValues(summary))
          setAcknowledged(null)
          setError(null)
        }
      }}
      title="Передать текущий этап агенту"
      returnFocusRef={trigger}
      pending={busy}
      dirty={dirty && !uncertain}
      error={error}
      onSubmit={submit}
      actions={
        <>
          {checkKey && (
            <Button
              type="button"
              variant="outline"
              onClick={() => lookup.mutate({ key: checkKey })}
            >
              Проверить результат
            </Button>
          )}
          {uncertain && (
            <Button
              type="button"
              variant="outline"
              disabled={disabled || !authenticated}
              onClick={() => void repeatOriginal()}
            >
              Повторить исходный запрос
            </Button>
          )}
          {current?.can_cancel && (
            <Button
              type="button"
              variant="outline"
              disabled={disabled || query.isFetching}
              onClick={() => void cancelCurrent()}
            >
              Отменить запрос
            </Button>
          )}
          {!current && !uncertain && (
            <Button type="submit" disabled={!canSubmit}>
              Передать агенту
            </Button>
          )}
        </>
      }
    >
      {!authenticated ? (
        <ErrorState message="Для проверки нужен вход в платформу." />
      ) : query.isPending ? (
        <LoadingState message="Проверяем готовность исполнения…" />
      ) : query.error ? (
        <ErrorState message={message(query.error)} onRetry={() => void query.refetch()} />
      ) : view ? (
        <div className="min-w-0 space-y-4">
          <p className="text-sm text-text-muted">
            {stageLabels[view.state.stage]} · {lifecycleStatusLabels[view.state.status]}
          </p>
          {query.isFetching && <LoadingState message="Обновляем состояние…" />}
          {current ? (
            <Notice role="status" tone="neutral" appearance="inline">
              Запрос сохранён и ожидает назначения. Стадия задачи сохраняется.
            </Notice>
          ) : request.data && query.isFetching ? (
            <Notice role="status" tone="neutral" appearance="inline">
              Запрос принят. Загружаем актуальное состояние.
            </Notice>
          ) : null}
          {!current &&
            view.blockers.map((reason) => (
              <Notice key={reason} tone="neutral" appearance="inline">
                {blockerLabels[reason]}
              </Notice>
            ))}
          {lookup.isError && (
            <ErrorState
              message={
                lookup.error instanceof HandoffRequestError && lookup.error.status === 404
                  ? 'Исходная операция пока не найдена. Можно проверить ещё раз или повторить тот же запрос.'
                  : message(lookup.error)
              }
            />
          )}
          {lookup.data?.cancellation && (
            <Notice role="status" tone="neutral" appearance="inline">
              Отмена исходного запроса подтверждена.
            </Notice>
          )}
          {!current && !view.blockers.length && (
            <>
              <FormField id={`${id}-goal`} label="Цель" required>
                {(attrs) => (
                  <Textarea
                    {...attrs}
                    rows={2}
                    value={values.goal}
                    onChange={(event) =>
                      setValue('goal', event.target.value, { shouldDirty: true })
                    }
                    disabled={!ready || uncertain}
                  />
                )}
              </FormField>
              <p className="text-xs text-text-muted">
                Для черновика или бэклога достаточно цели. Для следующих этапов заполните материалы;
                каждый пункт — с новой строки.
              </p>
              <Disclosure
                summary="Материалы этапа"
                open={view.state.stage !== 'Draft' && view.state.stage !== 'Backlog'}
              >
                <div className="min-w-0 space-y-3">{mainFields.map(renderField)}</div>
              </Disclosure>
              <Disclosure summary="Дополнительные материалы">
                <div className="min-w-0 space-y-3">{extraFields.map(renderField)}</div>
              </Disclosure>
              <div className="flex items-start gap-2">
                <Checkbox
                  id={`${id}-confirm`}
                  checked={acknowledged === pin}
                  disabled={!ready || uncertain}
                  onCheckedChange={(value) => setAcknowledged(value === true ? pin : null)}
                />
                <label htmlFor={`${id}-confirm`} className="text-sm">
                  Подтверждаю материалы задачи и передачу текущего этапа агенту.
                </label>
              </div>
            </>
          )}
        </div>
      ) : null}
    </FormDialog>
  )
}
