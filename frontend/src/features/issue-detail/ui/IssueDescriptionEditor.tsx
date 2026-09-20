import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Pencil, Check, X } from 'lucide-react'
import type { Issue } from '@/api/issue'
import { Button } from '@sdlc/ui/ui'
import { Textarea } from '@sdlc/ui/ui'

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
        <div className="mb-2 flex items-start justify-between">
          <h1 className="min-w-0 break-words text-2xl font-semibold text-text-primary">
            {issue.summary}
          </h1>
          <Button
            variant="ghost"
            size="icon"
            className="h-10 w-10 shrink-0 opacity-100 sm:opacity-0 sm:group-hover:opacity-100 sm:group-focus-within:opacity-100 lg:h-9 lg:w-9"
            onClick={(e) => {
              e.stopPropagation()
              startEdit()
            }}
            disabled={disabled}
            aria-label={t('common.edit')}
          >
            <Pencil className="h-4 w-4" />
          </Button>
        </div>
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
                className="mt-1 h-10 px-0 lg:hidden"
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
      <div>
        <input
          {...register('summary')}
          aria-label={t('issueCreate.summary')}
          className="w-full rounded-md border border-border bg-surface px-3 py-2 text-2xl font-semibold text-text-primary focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent"
        />
        {errors.summary && (
          <p className="mt-1 text-xs text-red-500">{t('issue.summaryRequired')}</p>
        )}
      </div>
      <Textarea
        {...register('description')}
        rows={8}
        placeholder={t('issue.descriptionPlaceholder')}
      />
      {saveError && (
        <p role="alert" className="text-sm text-danger">
          {saveError}
        </p>
      )}
      <div className="flex gap-2">
        <Button type="submit" size="sm" className="h-10 lg:h-8" disabled={disabled || isSubmitting}>
          <Check className="mr-1 h-4 w-4" />
          {isSubmitting ? t('common.saving') : t('common.save')}
        </Button>
        <Button
          type="button"
          variant="secondary"
          size="sm"
          className="h-10 lg:h-8"
          onClick={cancel}
          disabled={disabled || isSubmitting}
        >
          <X className="mr-1 h-4 w-4" />
          {t('common.cancel')}
        </Button>
      </div>
    </form>
  )
}

function renderDescription(text: string) {
  const lines = text.split('\n')
  return (
    <div className="space-y-3 break-words text-sm text-text-secondary">
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
    </div>
  )
}
