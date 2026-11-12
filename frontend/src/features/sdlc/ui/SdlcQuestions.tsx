import { useId, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useScopedMutation } from '@/shared/api/useScopedMutation'
import { useTranslation } from 'react-i18next'
import {
  Button,
  Checkbox,
  ErrorState,
  Fieldset,
  FormField,
  LoadingState,
  RadioGroup,
  Textarea,
} from '@pdlc/ui/ui'
import {
  answerSdlcQuestion,
  getSdlcQuestions,
  SdlcRequestError,
  type SdlcAnswerCommand,
  type SdlcContext,
  type SdlcQuestion,
} from '@/api/sdlc'

export function SdlcQuestions({
  context,
  queryKey,
  enabled,
  onChanged,
}: {
  context: SdlcContext
  queryKey: readonly unknown[]
  enabled: boolean
  onChanged: () => Promise<void>
}) {
  const { t } = useTranslation()
  const questions = useQuery({
    queryKey: [...queryKey, 'questions', context.requirement_revision],
    queryFn: ({ signal }) => getSdlcQuestions(context, signal),
    enabled: enabled && context.permissions.can_answer,
    retry: false,
    gcTime: 0,
  })
  if (!enabled || !context.permissions.can_answer) return null
  if (questions.error)
    return (
      <ErrorState
        message={t(
          `sdlc.errors.${questions.error instanceof SdlcRequestError ? questions.error.status : 'unknown'}`,
          { defaultValue: t('sdlc.errors.unknown') },
        )}
        onRetry={() => void questions.refetch()}
      />
    )
  if (questions.isPending) return <LoadingState message={t('sdlc.questions.loading')} />
  const open = questions.data.filter((question) => question.state === 'open')
  if (!open.length)
    return <ErrorState message={t('sdlc.questions.notFound')} onRetry={() => void onChanged()} />
  return (
    <div className="mt-3 space-y-4">
      {open.map((question) => (
        <QuestionAnswer
          key={`${question.id}:${question.version}:${question.requirement_revision}`}
          question={question}
          context={context}
          onChanged={onChanged}
          enabled={
            !questions.isFetching && question.requirement_revision === context.requirement_revision
          }
        />
      ))}
    </div>
  )
}

function QuestionAnswer({
  question,
  context,
  enabled,
  onChanged,
}: {
  question: SdlcQuestion
  context: SdlcContext
  enabled: boolean
  onChanged: () => Promise<void>
}) {
  const { t } = useTranslation()
  const textId = useId()
  const [text, setText] = useState('')
  const [selected, setSelected] = useState<string[]>([])
  const [key] = useState(() => crypto.randomUUID())
  const answer = useScopedMutation({
    mutationFn: (command: SdlcAnswerCommand) => answerSdlcQuestion(context, question, command),
    retry: false,
    onSettled: onChanged,
  })
  const needsText =
    question.mode === 'text' ||
    question.options.some((option) => option.is_custom && selected.includes(option.id))
  const valid =
    (question.mode === 'text' || selected.length > 0) && (!needsText || text.trim().length > 0)
  const busy = !enabled || answer.isPending
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        if (enabled && context.permissions.can_answer && !answer.isPending && valid)
          answer.mutate({
            expected_question_version: question.version,
            requirement_revision: question.requirement_revision,
            selected_option_ids: [...selected],
            text: text || null,
            idempotency_key: key,
          })
      }}
    >
      <Fieldset legend={question.text} disabled={busy}>
        <p>{question.rationale}</p>
        {question.required && <p>{t('sdlc.questions.required')}</p>}
        {question.mode === 'single' && (
          <RadioGroup
            label={t('sdlc.questions.options')}
            value={selected[0] ?? ''}
            onValueChange={(value) => setSelected([value])}
            options={question.options.map((option) => ({
              value: option.id,
              label: option.consequences
                ? `${option.label} — ${option.consequences}`
                : option.label,
            }))}
          />
        )}
        {question.mode === 'multiple' &&
          question.options.map((option) => (
            <div key={option.id}>
              <Checkbox
                label={option.label}
                checked={selected.includes(option.id)}
                onCheckedChange={(checked) =>
                  setSelected((current) =>
                    checked === true
                      ? [...current, option.id]
                      : current.filter((id) => id !== option.id),
                  )
                }
              />
              <p>{option.consequences}</p>
            </div>
          ))}
        {(question.mode === 'text' || needsText) && (
          <FormField id={textId} label={t('sdlc.questions.answer')}>
            {(attrs) => (
              <Textarea
                {...attrs}
                value={text}
                onChange={(event) => setText(event.target.value)}
                required
                maxLength={100000}
              />
            )}
          </FormField>
        )}
        <Button type="submit" disabled={busy || !valid}>
          {t(answer.isPending ? 'sdlc.questions.saving' : 'sdlc.questions.submit')}
        </Button>
      </Fieldset>
      {answer.error && (
        <ErrorState
          message={t(
            `sdlc.errors.${answer.error instanceof SdlcRequestError ? answer.error.status : 'unknown'}`,
            { defaultValue: t('sdlc.errors.unknown') },
          )}
        />
      )}
    </form>
  )
}
