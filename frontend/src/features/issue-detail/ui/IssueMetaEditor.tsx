import { useTranslation } from 'react-i18next'
import { useMemo } from 'react'
import { Label, Select, Button, Notice } from '@pdlc/ui/ui'
import type { Issue } from '@/api/issue'
import type { Sprint } from '@/api/sprint'
import type { Board } from '@/api/board'
import { statusLabel } from '@/shared/lib/status-label'
import { useStatuses, useTransitions, useUsers } from '@/shared/api/hooks'

const priorities = ['Lowest', 'Low', 'Medium', 'High', 'Highest']

interface IssueMetaEditorProps {
  issue: Issue
  columns: Board['columns']
  sprints?: Sprint[]
  onChange: (patch: {
    status_id?: string
    priority?: string
    assignee_id?: string | null
    sprint_id?: string | null
  }) => void
  disabled?: boolean
}

export function IssueMetaEditor({
  issue,
  columns,
  sprints,
  onChange,
  disabled,
}: IssueMetaEditorProps) {
  const { t } = useTranslation()
  const usersQuery = useUsers()
  const statusesQuery = useStatuses()
  const transitionsQuery = useTransitions()

  // Available target statuses for the current status, per workflow transitions.
  // Falls back to all statuses (minus the current one) when transitions are unavailable.
  const statusOptions = useMemo(() => {
    const all =
      statusesQuery.data && statusesQuery.data.length > 0
        ? statusesQuery.data.map((s) => ({ id: s.id, name: s.name }))
        : columns.map((c) => ({ id: c.id, name: c.name }))
    const fromTransitions = (transitionsQuery.data ?? [])
      .filter((tr) => tr.from_status_id === issue.status_id)
      .map((tr) => tr.to_status_id)
    if (fromTransitions.length === 0) return all
    const allowed = new Set([issue.status_id, ...fromTransitions])
    return all.filter((s) => allowed.has(s.id))
  }, [statusesQuery.data, transitionsQuery.data, columns, issue.status_id])

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
        <Label htmlFor="issue-status" className="block">
          {t('issue.status')}
        </Label>
        <Select
          id="issue-status"
          value={issue.status_id}
          onValueChange={(nextValue) => onChange({ status_id: nextValue })}
          disabled={disabled}
          className="w-full"
          options={[
            ...(statusOptions.map((s) => ({
              value: String(s.id),
              label: String(statusLabel(s.name, t)),
            })) ?? []),
          ]}
        />
      </div>

      <div className="space-y-1.5">
        <Label htmlFor="issue-priority" className="block">
          {t('issue.priority')}
        </Label>
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
        <Label htmlFor="issue-assignee" className="block">
          {t('issue.assignee')}
        </Label>
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
          <Label htmlFor="issue-sprint" className="block">
            {t('issue.sprint')}
          </Label>
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
