import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Tag, Plus, X } from 'lucide-react'
import {
  useProjectLabels,
  useIssueLabels,
  useAttachLabel,
  useDetachLabel,
  useCreateLabel,
} from '@/shared/api/hooks'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'

const PALETTE = [
  '#ef4444',
  '#f97316',
  '#eab308',
  '#22c55e',
  '#3b82f6',
  '#8b5cf6',
  '#ec4899',
  '#6b7280',
]

export function LabelEditor({ issueId, projectKey }: { issueId: string; projectKey: string }) {
  const { t } = useTranslation()
  const { data: projectLabels = [] } = useProjectLabels(projectKey)
  const { data: issueLabels = [] } = useIssueLabels(issueId)
  const attach = useAttachLabel(issueId, projectKey)
  const detach = useDetachLabel(issueId, projectKey)
  const create = useCreateLabel(projectKey)
  const [creating, setCreating] = useState(false)
  const [newName, setNewName] = useState('')

  const issueLabelIds = new Set(issueLabels.map((l) => l.id))

  const onCreate = async () => {
    const name = newName.trim()
    if (!name) return
    const color = PALETTE[projectLabels.length % PALETTE.length] ?? '#6b7280'
    const label = await create.mutateAsync({ name, color })
    await attach.mutateAsync(label.id)
    setNewName('')
    setCreating(false)
  }

  return (
    <div className="space-y-2" data-testid="label-editor">
      <h3 className="flex items-center gap-2 text-sm font-semibold">
        <Tag className="h-4 w-4" aria-hidden />
        {t('labels.title')}
      </h3>

      {issueLabels.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {issueLabels.map((l) => (
            <span
              key={l.id}
              className="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium text-white"
              style={{ backgroundColor: l.color }}
              data-testid="issue-label"
            >
              {l.name}
              <button
                type="button"
                aria-label={t('labels.detach', { name: l.name })}
                onClick={() => detach.mutate(l.id)}
                className="rounded-full p-0.5 hover:bg-black/20"
              >
                <X className="h-3 w-3" aria-hidden />
              </button>
            </span>
          ))}
        </div>
      )}
      {issueLabels.length === 0 && (
        <p className="text-xs text-muted-foreground">{t('labels.none')}</p>
      )}

      {creating ? (
        <div className="flex gap-1">
          <Input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder={t('labels.namePlaceholder')}
            className="h-8 text-xs"
            data-testid="label-name-input"
            onKeyDown={(e) => {
              if (e.key === 'Enter') void onCreate()
            }}
          />
          <Button type="button" size="sm" className="h-8" onClick={() => void onCreate()}>
            {t('labels.add')}
          </Button>
        </div>
      ) : (
        <div className="flex flex-wrap gap-1">
          {projectLabels
            .filter((l) => !issueLabelIds.has(l.id))
            .map((l) => (
              <button
                key={l.id}
                type="button"
                onClick={() => attach.mutate(l.id)}
                className="rounded-full px-2 py-0.5 text-xs font-medium text-white opacity-70 transition hover:opacity-100"
                style={{ backgroundColor: l.color }}
              >
                + {l.name}
              </button>
            ))}
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs"
            onClick={() => setCreating(true)}
            aria-label={t('labels.create')}
          >
            <Plus className="mr-1 h-3 w-3" aria-hidden />
            {t('labels.create')}
          </Button>
        </div>
      )}
    </div>
  )
}
