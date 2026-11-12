import { useTranslation } from 'react-i18next'
import { useMemo } from 'react'
import { Select, Button, Notice } from '@pdlc/ui/ui'
import type { Issue } from '@/api/issue'
import type { Sprint } from '@/api/sprint'
import { useUsers } from '@/shared/api/hooks'

const priorities = ['Lowest', 'Low', 'Medium', 'High', 'Highest']

interface IssueMetaEditorProps {
  issue: Issue
  sprints?: Sprint[]
  onChange: (patch: {
    priority?: string
    assignee_id?: string | null
    sprint_id?: string | null
  }) => void
  disabled?: boolean
}

export function IssueMetaEditor({ issue, sprints, onChange, disabled }: IssueMetaEditorProps) {
  const { t } = useTranslation()
  const usersQuery = useUsers()
  const assigneeOptions = useMemo<
    Array<{ value: string; label: string; disabled?: boolean }>
  >(() => {
    const list = usersQuery.data ?? []
    const hasCurrentAssignee =
      !!issue.assignee_id && list.some((user) => user.id === issue.assignee_id)
    return [
      { value: '', label: t('issue.unassigned') },
      ...list.map((u) => ({
        value: u.id,
        label: u.display_name || u.username,
      })),
      ...(issue.assignee_id && !hasCurrentAssignee
        ? [
            {
              value: issue.assignee_id,
              label: issue.assignee_name ?? issue.assignee_id,
              disabled: true,
            },
          ]
        : []),
    ]
  }, [issue.assignee_id, issue.assignee_name, t, usersQuery.data])

  return (
    <div className="space-y-4 text-sm">
      <div className="space-y-1.5">
        <label htmlFor="issue-priority" className="block text-text-muted">
          {t('issue.priority')}
        </label>
        <Select
          id="issue-priority"
          value={issue.priority}
          onValueChange={(nextValue) => onChange({ priority: nextValue })}
          disabled={disabled}
          className="w-full"
          options={[
            ...(priorities.map((p) => ({
              value: String(p),
              label: String(t(`priority.${p.toLowerCase()}`, { defaultValue: p })),
            })) ?? []),
          ]}
        />
      </div>

      <div className="space-y-1.5">
        <label htmlFor="issue-assignee" className="block text-text-muted">
          {t('issue.assignee')}
        </label>
        <Select
          id="issue-assignee"
          value={issue.assignee_id ?? ''}
          onValueChange={(nextValue) => onChange({ assignee_id: nextValue || null })}
          disabled={disabled || (!usersQuery.data && (usersQuery.isLoading || usersQuery.isError))}
          className="w-full"
          options={[
            ...(assigneeOptions.map((opt) => ({
              value: String(opt.value),
              label: String(opt.label),
              disabled: opt.disabled,
            })) ?? []),
          ]}
        />
        {usersQuery.isError && (
          <Notice
            role="alert"
            tone="danger"
            appearance="inline"
            actions={
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={usersQuery.isFetching}
                onClick={() => void usersQuery.refetch()}
              >
                {t('common.retry')}
              </Button>
            }
          >
            {t(
              usersQuery.data
                ? 'issue.assigneeDirectoryRefreshError'
                : 'issue.assigneeDirectoryError',
            )}
          </Notice>
        )}
      </div>

      {sprints && (
        <div className="space-y-1.5">
          <label htmlFor="issue-sprint" className="block text-text-muted">
            {t('issue.sprint')}
          </label>
          <Select
            id="issue-sprint"
            value={issue.sprint_id ?? ''}
            onValueChange={(nextValue) => onChange({ sprint_id: nextValue || null })}
            disabled={disabled}
            className="w-full"
            options={[
              { value: '', label: String(t('issue.noSprint')) },
              ...(sprints.map((s) => ({ value: String(s.id), label: String(s.name) })) ?? []),
            ]}
          />
        </div>
      )}
    </div>
  )
}
