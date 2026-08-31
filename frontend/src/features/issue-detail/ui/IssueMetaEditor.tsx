import { useTranslation } from 'react-i18next'
import { useMemo } from 'react'
import type { Issue } from '@/api/issue'
import type { Sprint } from '@/api/sprint'
import type { Board } from '@/api/board'
import {
  useProjectMembers,
  useProjects,
  useStatuses,
  useTransitions,
  useUsers,
} from '@/shared/api/hooks'

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
  const projectsQuery = useProjects()
  const projectMembersQuery = useProjectMembers(issue.project_key)
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

  const currentProject = useMemo(
    () => (projectsQuery.data ?? []).find((project) => project.key === issue.project_key),
    [issue.project_key, projectsQuery.data],
  )

  const assigneeOptions = useMemo<
    Array<{ value: string; label: string; disabled?: boolean }>
  >(() => {
    const allowedIds = new Set((projectMembersQuery.data?.members ?? []).map((m) => m.user_id))
    if (currentProject?.owner_id) {
      allowedIds.add(currentProject.owner_id)
    }
    const list = (usersQuery.data ?? []).filter((user) => allowedIds.has(user.id))
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
  }, [
    currentProject?.owner_id,
    issue.assignee_id,
    issue.assignee_name,
    projectMembersQuery.data?.members,
    t,
    usersQuery.data,
  ])

  const selectClass =
    'w-full rounded-md border border-border bg-surface px-3 py-2 text-sm text-text-primary focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent disabled:opacity-50'

  return (
    <div className="space-y-4 text-sm">
      <div className="space-y-1.5">
        <label htmlFor="issue-status" className="block text-text-muted">
          {t('issue.status')}
        </label>
        <select
          id="issue-status"
          value={issue.status_id}
          onChange={(e) => onChange({ status_id: e.target.value })}
          disabled={disabled}
          className={selectClass}
        >
          {statusOptions.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
        </select>
      </div>

      <div className="space-y-1.5">
        <label htmlFor="issue-priority" className="block text-text-muted">
          {t('issue.priority')}
        </label>
        <select
          id="issue-priority"
          value={issue.priority}
          onChange={(e) => onChange({ priority: e.target.value })}
          disabled={disabled}
          className={selectClass}
        >
          {priorities.map((p) => (
            <option key={p} value={p}>
              {t(`priority.${p.toLowerCase()}`, { defaultValue: p })}
            </option>
          ))}
        </select>
      </div>

      <div className="space-y-1.5">
        <label htmlFor="issue-assignee" className="block text-text-muted">
          {t('issue.assignee')}
        </label>
        <select
          id="issue-assignee"
          value={issue.assignee_id ?? ''}
          onChange={(e) => onChange({ assignee_id: e.target.value || null })}
          disabled={disabled || usersQuery.isLoading || projectMembersQuery.isLoading}
          className={selectClass}
        >
          {assigneeOptions.map((opt) => (
            <option key={opt.value} value={opt.value} disabled={opt.disabled}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>

      {sprints && (
        <div className="space-y-1.5">
          <label htmlFor="issue-sprint" className="block text-text-muted">
            {t('issue.sprint')}
          </label>
          <select
            id="issue-sprint"
            value={issue.sprint_id ?? ''}
            onChange={(e) => onChange({ sprint_id: e.target.value || null })}
            disabled={disabled}
            className={selectClass}
          >
            <option value="">{t('issue.noSprint')}</option>
            {sprints.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </div>
      )}
    </div>
  )
}
