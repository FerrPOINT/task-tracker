import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Pencil, Check, X } from 'lucide-react'
import type { Issue } from '@/api/issue'
import { Button } from '@/shared/ui/button'
import { Textarea } from '@/shared/ui/textarea'

const schema = z.object({
  summary: z.string().min(1),
  description: z.string(),
})

type FormData = z.infer<typeof schema>

interface IssueDescriptionEditorProps {
  issue: Issue
  onSubmit: (patch: { summary: string; description: string | null }) => void
  disabled?: boolean
}

export function IssueDescriptionEditor({
  issue,
  onSubmit,
  disabled,
}: IssueDescriptionEditorProps) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  const {
    register,
    handleSubmit,
    reset,
    formState: { errors },
  } = useForm<FormData>({
    resolver: zodResolver(schema),
    defaultValues: {
      summary: issue.summary,
      description: issue.description ?? '',
    },
  })

  const startEdit = () => {
    reset({ summary: issue.summary, description: issue.description ?? '' })
    setEditing(true)
  }

  const cancel = () => {
    setEditing(false)
    reset()
  }

  const submit = handleSubmit((data) => {
    onSubmit({
      summary: data.summary,
      description: data.description.trim() || null,
    })
    setEditing(false)
  })

  if (!editing) {
    return (
      <div className="group cursor-pointer" onClick={startEdit}>
        <div className="mb-2 flex items-start justify-between">
          <h1 className="text-2xl font-semibold text-text-primary">{issue.summary}</h1>
          <Button
            variant="ghost"
            size="icon"
            className="opacity-0 group-hover:opacity-100"
            onClick={(e) => {
              e.stopPropagation()
              startEdit()
            }}
            disabled={disabled}
          >
            <Pencil className="h-4 w-4" />
          </Button>
        </div>
        {issue.description ? (
          renderDescription(issue.description)
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
      <div className="flex gap-2">
        <Button type="submit" size="sm" disabled={disabled}>
          <Check className="mr-1 h-4 w-4" />
          {t('common.save')}
        </Button>
        <Button type="button" variant="secondary" size="sm" onClick={cancel} disabled={disabled}>
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
    <div className="space-y-3 text-sm text-text-secondary">
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
