import { useTranslation } from 'react-i18next'
import { useMemo } from 'react'
import type { Issue } from '@/api/issue'
import type { Board } from '@/api/board'
import { useCurrentUser, useUsers } from '@/shared/api/hooks'

const priorities = ['Low', 'Medium', 'High', 'Critical']

interface IssueMetaEditorProps {
  issue: Issue
  columns: Board['columns']
  onChange: (patch: {
    status_id?: string
    priority?: string
    assignee_id?: string | null
  }) => void
  disabled?: boolean
}

export function IssueMetaEditor({ issue, columns, onChange, disabled }: IssueMetaEditorProps) {
  const { t } = useTranslation()
  const currentUser = useCurrentUser()
  const usersQuery = useUsers()

  const assigneeOptions = useMemo(() => {
    const initial = usersQuery.data ?? []
    const list = [...initial]
    if (currentUser.data?.id && !list.find((u) => u.id === currentUser.data!.id)) {
      list.unshift(currentUser.data)
    }
    return [
      { value: '', label: t('issue.unassigned') },
      ...list.map((u) => ({
        value: u.id,
        label: u.display_name || u.username || u.email,
      })),
    ]
  }, [usersQuery.data, currentUser.data, t])

  const selectClass =
    'w-full rounded-md border border-border bg-surface px-3 py-2 text-sm text-text-primary focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent disabled:opacity-50'

  return (
    <div className="space-y-4 text-sm">
      <div className="space-y-1.5">
        <label className="block text-text-muted">{t('issue.status')}</label>
        <select
          value={issue.status_id}
          onChange={(e) => onChange({ status_id: e.target.value })}
          disabled={disabled}
          className={selectClass}
        >
          {columns.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
      </div>

      <div className="space-y-1.5">
        <label className="block text-text-muted">{t('issue.priority')}</label>
        <select
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
        <label className="block text-text-muted">{t('issue.assignee')}</label>
        <select
          value={issue.assignee_id ?? ''}
          onChange={(e) => onChange({ assignee_id: e.target.value || null })}
          disabled={disabled || usersQuery.isLoading}
          className={selectClass}
        >
          {assigneeOptions.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>
    </div>
  )
}
