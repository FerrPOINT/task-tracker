import {
  ActionGroup,
  Input,
  Button,
  Textarea,
  Notice,
  FormField,
  PageHeader,
  Prose,
} from '@pdlc/ui/ui'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Pencil, Check, X } from 'lucide-react'
import type { Issue } from '@/api/issue'

const schema = z.object({
  summary: z.string().min(1),
  description: z.string(),
})

type FormData = z.infer<typeof schema>

interface IssueDescriptionEditorProps {
  issue: Issue
  onSubmit: (patch: { summary: string; description: string | null }) => Promise<unknown>
  disabled?: boolean
}

export function IssueDescriptionEditor({ issue, onSubmit, disabled }: IssueDescriptionEditorProps) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  const [descriptionExpanded, setDescriptionExpanded] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const descriptionId = useId()
  const descriptionLines = issue.description?.split('\n') ?? []
  const compactDescription = descriptionLines.slice(0, 4).join('\n').slice(0, 240).trimEnd()
  const hasLongDescription =
    issue.description != null &&
    (issue.description.length > 280 || descriptionLines.length > 5) &&
    compactDescription.length < issue.description.length
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
    setEditing(true)
  }

  const cancel = () => {
    setEditing(false)
    reset()
  }

  const submit = handleSubmit(async (data) => {
    setSaveError(null)
    try {
      await onSubmit({
        summary: data.summary,
        description: data.description.trim() || null,
      })
      setEditing(false)
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : t('common.error'))
    }
  })

  if (!editing) {
    return (
      <div className="group min-w-0 cursor-pointer" onClick={startEdit}>
        <PageHeader
          title={issue.summary}
          actions={
            <Button
              variant="ghost"
              size="icon"
              className="shrink-0 opacity-100 xl:opacity-0 xl:group-hover:opacity-100 xl:group-focus-within:opacity-100"
              onClick={(e) => {
                e.stopPropagation()
                startEdit()
              }}
              disabled={disabled}
              aria-label={t('common.edit')}
            >
              <Pencil className="h-4 w-4" />
            </Button>
          }
        />
        {issue.description ? (
          hasLongDescription ? (
            <>
              <div id={descriptionId} className="lg:hidden">
                {renderDescription(
                  descriptionExpanded ? issue.description : `${compactDescription}…`,
                )}
              </div>
              <div className="hidden lg:block">{renderDescription(issue.description)}</div>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="mt-1 lg:hidden"
                aria-controls={descriptionId}
                aria-expanded={descriptionExpanded}
                onClick={(event) => {
                  event.stopPropagation()
                  setDescriptionExpanded((value) => !value)
                }}
              >
                {t(descriptionExpanded ? 'issue.showLessDescription' : 'issue.showMoreDescription')}
              </Button>
            </>
          ) : (
            renderDescription(issue.description)
          )
        ) : (
          <p className="text-sm text-text-muted">{t('issue.noDescription')}</p>
        )}
      </div>
    )
  }

  return (
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
      {saveError && (
        <Notice role="alert" tone="danger" appearance="inline">
          {saveError}
        </Notice>
      )}
      <ActionGroup align="start">
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
}

function renderDescription(text: string) {
  const lines = text.split('\n')
  return (
    <Prose>
      {lines.map((line, idx) => {
        if (line.startsWith('· ')) {
          return (
            <ul key={idx}>
              <li>{line.slice(2)}</li>
            </ul>
          )
        }
        if (line.startsWith('# ')) {
          return <h3 key={idx}>{line.slice(2)}</h3>
        }
        if (line.trim() === '') {
          return <div key={idx} className="h-2" />
        }
        return <p key={idx}>{line}</p>
      })}
    </Prose>
  )
}
