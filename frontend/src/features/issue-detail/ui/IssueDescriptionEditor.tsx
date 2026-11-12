import {
  ActionGroup,
  Prose,
  Input,
  Button,
  Textarea,
  Notice,
  FormField,
  PageHeader,
  SectionHeader,
} from '@pdlc/ui/ui'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Pencil, Check, X } from 'lucide-react'
import type { Issue } from '@/api/issue'
import type { RequirementsRevision } from '@/api/sdlc'

const schema = z.object({
  summary: z.string().min(1),
  description: z.string(),
})

type FormData = z.infer<typeof schema>

interface IssueDescriptionEditorProps {
  issue: Issue
  onSubmit: (patch: { summary: string; description?: string | null }) => Promise<unknown>
  disabled?: boolean
  requirements?: RequirementsRevision | null
}

export function IssueDescriptionEditor({
  issue,
  onSubmit,
  disabled,
  requirements,
}: IssueDescriptionEditorProps) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState<'title' | 'issue' | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)
  const descriptionId = useId()
  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<FormData>({
    resolver: zodResolver(schema),
    defaultValues: {
      summary: issue.summary,
      description: issue.description ?? '',
    },
  })

  const startEdit = () => {
    reset({ summary: issue.summary, description: issue.description ?? '' })
    setSaveError(null)
    setEditing(requirements ? 'title' : 'issue')
  }

  const cancel = () => {
    setEditing(null)
    reset()
  }

  const submit = handleSubmit(async (data) => {
    setSaveError(null)
    try {
      await onSubmit(
        editing === 'title'
          ? { summary: data.summary }
          : { summary: data.summary, description: data.description.trim() || null },
      )
      setEditing(null)
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : t('common.error'))
    }
  })

  const editor = (
    <form onSubmit={submit} className="space-y-3">
      <FormField
        id={`${descriptionId}-summary`}
        label={t('issueCreate.summary')}
        error={errors.summary ? t('issue.summaryRequired') : undefined}
      >
        {(attrs) => (
          <Input
            {...attrs}
            {...register('summary')}
            variant="heading"
            aria-label={t('issueCreate.summary')}
            className="w-full"
          />
        )}
      </FormField>
      {editing === 'issue' && (
        <FormField id={descriptionId} label={t('issueCreate.description')}>
          {(attrs) => (
            <Textarea
              {...attrs}
              {...register('description')}
              rows={8}
              placeholder={t('issue.descriptionPlaceholder')}
            />
          )}
        </FormField>
      )}
      {saveError && (
        <Notice role="alert" tone="danger" appearance="inline">
          {saveError}
        </Notice>
      )}
      <ActionGroup>
        <Button type="submit" size="sm" disabled={disabled || isSubmitting}>
          <Check className="mr-1 h-4 w-4" />
          {isSubmitting ? t('common.saving') : t('common.save')}
        </Button>
        <Button
          type="button"
          variant="secondary"
          size="sm"
          onClick={cancel}
          disabled={disabled || isSubmitting}
        >
          <X className="mr-1 h-4 w-4" />
          {t('common.cancel')}
        </Button>
      </ActionGroup>
    </form>
  )

  if (editing !== 'issue') {
    return (
      <div className="grid min-w-0 gap-4">
        {editing === 'title' ? (
          editor
        ) : (
          <div className="flex min-w-0 items-start gap-2">
            <div className="min-w-0 flex-1">
              <PageHeader title={issue.summary} />
            </div>
            <Button
              variant="ghost"
              size="icon"
              className="shrink-0"
              onClick={startEdit}
              disabled={disabled}
              aria-label={t(requirements ? 'issue.editTitle' : 'issue.editDescription')}
            >
              <Pencil className="h-4 w-4" />
            </Button>
          </div>
        )}
        <section aria-labelledby={`${descriptionId}-heading`} className="grid min-w-0 gap-3">
          <SectionHeader
            level={2}
            headingId={`${descriptionId}-heading`}
            title={t('issueCreate.description')}
          />
          {requirements ? (
            <>
              <p className="text-sm text-text-muted">
                {t('sdlc.requirements')} · {t('sdlc.revision')} {requirements.revision}
              </p>
              <p className="text-sm text-text-secondary whitespace-pre-wrap [overflow-wrap:anywhere]">
                {requirements.goal}
              </p>
              {(['scope', 'acceptance_criteria'] as const).map((field) => (
                <div key={field} className="grid min-w-0 gap-2">
                  <SectionHeader level={3} title={t(`sdlc.document.${field}`)} />
                  <ul className="list-disc space-y-1 pl-5 text-sm text-text-secondary [overflow-wrap:anywhere]">
                    {requirements[field].map((item, index) => (
                      <li key={index}>{item}</li>
                    ))}
                  </ul>
                </div>
              ))}
            </>
          ) : issue.description ? (
            renderDescription(issue.description)
          ) : (
            <p className="text-sm text-text-muted">{t('issue.noDescription')}</p>
          )}
        </section>
      </div>
    )
  }

  return editor
}

function renderDescription(text: string) {
  const lines = text.split('\n')
  return (
    <Prose className="min-w-0 space-y-3 text-sm text-text-secondary [overflow-wrap:anywhere]">
      {lines.map((line, idx) => {
        if (line.startsWith('· ')) {
          return (
            <ul key={idx} className="ml-5 list-disc">
              <li>{line.slice(2)}</li>
            </ul>
          )
        }
        if (line.startsWith('# ')) {
          return (
            <h3 key={idx} className="text-base font-semibold text-text-primary">
              {line.slice(2)}
            </h3>
          )
        }
        if (line.trim() === '') {
          return <div key={idx} className="h-2" />
        }
        return <p key={idx}>{line}</p>
      })}
    </Prose>
  )
}
