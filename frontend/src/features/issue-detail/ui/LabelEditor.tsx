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
import {
  SectionHeader,
  Badge,
  Button,
  Input,
  toast,
  Notice,
  ResourceStateView,
  resourceErrorState,
  EmptyState,
  Fieldset,
  type ResourceState,
} from '@pdlc/ui/ui'

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

export { readableForeground as labelForeground } from '@pdlc/ui/lib'

export function LabelEditor({ issueId, projectKey }: { issueId: string; projectKey: string }) {
  return (
    <IssueLabelEditor key={`${projectKey}:${issueId}`} issueId={issueId} projectKey={projectKey} />
  )
}

function IssueLabelEditor({ issueId, projectKey }: { issueId: string; projectKey: string }) {
  const { t } = useTranslation()
  const project = useProjectLabels(projectKey)
  const issue = useIssueLabels(issueId)
  const projectLabels = project.data ?? []
  const issueLabels = issue.data ?? []
  const hasData = project.data !== undefined && issue.data !== undefined
  const retryReads = () => {
    void project.refetch()
    void issue.refetch()
  }
  const failures = [project.error, issue.error]
    .filter(Boolean)
    .map((error) => resourceErrorState(error, retryReads, { hasData }))
  // Both reads are needed to identify already-attached labels before changing them.
  const state: ResourceState =
    failures.find((value) => value.kind === 'permission-denied') ??
    failures.find((value) => value.kind === 'not-found') ??
    failures.find((value) => value.kind === 'error') ??
    failures[0] ??
    (!hasData ? { kind: 'loading', message: t('common.loading') } : { kind: 'ready' })
  const canEdit = state.kind === 'ready'
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
    if (!canEdit || !name) return
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
      <SectionHeader
        level={3}
        title={
          <>
            <Tag className="h-4 w-4" aria-hidden />
            {t('labels.title')}
          </>
        }
      />

      <ResourceStateView state={state} scope="local">
        <Fieldset legend={t('labels.title')} legendVisuallyHidden disabled={!canEdit}>
          {issueLabels.length > 0 && (
            <div className="flex flex-wrap gap-1">
              {issueLabels.map((l) => (
                <Badge key={l.id} subjectColor={l.color} data-testid="issue-label">
                  {l.name}
                  <Button
                    variant="subject"
                    size="icon"
                    type="button"
                    aria-label={t('labels.detach', { name: l.name })}
                    onClick={() =>
                      canEdit &&
                      detach.mutate(l.id, { onError: (error) => toast.error(error.message) })
                    }
                  >
                    <X className="h-3 w-3" aria-hidden />
                  </Button>
                </Badge>
              ))}
            </div>
          )}
          {issueLabels.length === 0 && <EmptyState message={t('labels.none')} />}

          {creating ? (
            <div className="space-y-1">
              <div className="flex gap-1">
                <Input
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  aria-label={t('labels.nameLabel')}
                  placeholder={t('labels.namePlaceholder')}
                  className="min-w-0 flex-1"
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
                  disabled={create.isPending || attach.isPending}
                  onClick={() => void onCreate()}
                >
                  {create.isPending || attach.isPending ? t('common.loading') : t('labels.add')}
                </Button>
              </div>
              {createError && (
                <Notice role="alert" tone="danger" appearance="inline">
                  {createError}
                </Notice>
              )}
            </div>
          ) : (
            <div className="flex flex-wrap gap-1">
              {projectLabels
                .filter((l) => !issueLabelIds.has(l.id))
                .map((l) => (
                  <Button
                    variant="subject"
                    size="sm"
                    key={l.id}
                    type="button"
                    onClick={() =>
                      canEdit &&
                      attach.mutate(l.id, { onError: (error) => toast.error(error.message) })
                    }
                    subjectColor={l.color}
                  >
                    + {l.name}
                  </Button>
                ))}
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => setCreating(true)}
                aria-label={t('labels.create')}
              >
                <Plus className="h-3 w-3" aria-hidden />
                {t('labels.create')}
              </Button>
            </div>
          )}
        </Fieldset>
      </ResourceStateView>
    </div>
  )
}
