import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Tag, Plus, X } from 'lucide-react'
import {
  useProjectLabels,
  useIssueLabels,
  useAttachLabel,
  useDetachLabel,
  useCreateLabel,
} from '@/shared/api/hooks'
import { Button } from '@sdlc/ui/ui'
import { Input } from '@sdlc/ui/ui'
import { toast } from 'sonner'

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

export function labelForeground(color: string): '#000000' | '#ffffff' {
  if (!/^#[0-9a-f]{6}$/i.test(color)) return '#000000'
  const channels = [1, 3, 5].map((index) => {
    const value = parseInt(color.slice(index, index + 2), 16) / 255
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
  })
  const luminance = 0.2126 * channels[0]! + 0.7152 * channels[1]! + 0.0722 * channels[2]!
  return (luminance + 0.05) / 0.05 >= 1.05 / (luminance + 0.05) ? '#000000' : '#ffffff'
}

export function LabelEditor({ issueId, projectKey }: { issueId: string; projectKey: string }) {
  const { t } = useTranslation()
  const { data: projectLabels = [] } = useProjectLabels(projectKey)
  const { data: issueLabels = [] } = useIssueLabels(issueId)
  const attach = useAttachLabel(issueId, projectKey)
  const detach = useDetachLabel(issueId, projectKey)
  const create = useCreateLabel(projectKey)
  const [creating, setCreating] = useState(false)
  const [newName, setNewName] = useState('')
  const [createError, setCreateError] = useState<string | null>(null)
  const createdLabel = useRef<{ name: string; id: string } | null>(null)

  const issueLabelIds = new Set(issueLabels.map((l) => l.id))

  const onCreate = async () => {
    const name = newName.trim()
    if (!name) return
    const color = PALETTE[projectLabels.length % PALETTE.length] ?? '#6b7280'
    setCreateError(null)
    try {
      let labelId = projectLabels.find((item) => item.name === name)?.id
      if (!labelId && createdLabel.current?.name === name) labelId = createdLabel.current.id
      if (!labelId) {
        const label = await create.mutateAsync({ name, color })
        labelId = label.id
        createdLabel.current = { name, id: labelId }
      }
      await attach.mutateAsync(labelId)
      createdLabel.current = null
      setNewName('')
      setCreating(false)
      toast.success(t('common.saved'))
    } catch (error) {
      setCreateError(
        error instanceof Error
          ? error.message
          : t('labels.createFailed', 'Не удалось добавить метку'),
      )
    }
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
              className="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium"
              style={{ backgroundColor: l.color, color: labelForeground(l.color) }}
              data-testid="issue-label"
            >
              {l.name}
              <button
                type="button"
                aria-label={t('labels.detach', { name: l.name })}
                onClick={() =>
                  detach.mutate(l.id, { onError: (error) => toast.error(error.message) })
                }
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
        <div className="space-y-1">
          <div className="flex gap-1">
            <Input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder={t('labels.namePlaceholder')}
              className="h-8 text-xs"
              data-testid="label-name-input"
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault()
                  void onCreate()
                }
              }}
            />
            <Button
              type="button"
              size="sm"
              className="h-8"
              disabled={create.isPending || attach.isPending}
              onClick={() => void onCreate()}
            >
              {create.isPending || attach.isPending ? t('common.loading') : t('labels.add')}
            </Button>
          </div>
          {createError && (
            <p role="alert" className="text-xs text-danger">
              {createError}
            </p>
          )}
        </div>
      ) : (
        <div className="flex flex-wrap gap-1">
          {projectLabels
            .filter((l) => !issueLabelIds.has(l.id))
            .map((l) => (
              <button
                key={l.id}
                type="button"
                onClick={() =>
                  attach.mutate(l.id, { onError: (error) => toast.error(error.message) })
                }
                className="rounded-full px-2 py-0.5 text-xs font-medium transition hover:brightness-110"
                style={{ backgroundColor: l.color, color: labelForeground(l.color) }}
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
