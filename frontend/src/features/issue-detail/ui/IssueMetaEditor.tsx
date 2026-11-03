import { useTranslation } from 'react-i18next'
import { useEffect, useMemo, useRef, useState } from 'react'
import {
  Button,
  Notice,
  ResourceListDescription,
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
} from '@pdlc/ui/ui'
import { ChevronDown } from 'lucide-react'
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

type MetaOption = { value: string; label: string; disabled?: boolean }

function MetaChoice({
  label,
  value,
  fallback,
  options,
  onChange,
  disabled = false,
}: {
  label: string
  value: string
  fallback?: string
  options: MetaOption[]
  onChange: (value: string) => void
  disabled?: boolean
}) {
  const [open, setOpen] = useState(false)
  const trigger = useRef<HTMLButtonElement>(null)
  const returnFocus = useRef(false)
  const selected = options.find((option) => option.value === value)?.label ?? fallback ?? value
  useEffect(() => {
    if (disabled) setOpen(false)
    else if (returnFocus.current) {
      returnFocus.current = false
      if (document.activeElement === document.body) trigger.current?.focus()
    }
  }, [disabled])
  return (
    <>
      <dt className="min-w-0">
        <ResourceListDescription>{label}</ResourceListDescription>
      </dt>
      <dd className="min-w-0">
        <DropdownMenu open={open} onOpenChange={(next) => setOpen(next && !disabled)}>
          <DropdownMenuTrigger asChild disabled={disabled}>
            <Button
              ref={trigger}
              type="button"
              variant="ghost"
              disabled={disabled}
              className="max-w-full justify-start"
              aria-label={`${label}: ${selected}`}
              title={selected}
            >
              <span className="min-w-0 truncate">{selected}</span>
              <ChevronDown className="h-3.5 w-3.5 shrink-0" aria-hidden />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="start"
            aria-label={label}
            onCloseAutoFocus={(event) => {
              if (disabled) {
                event.preventDefault()
                returnFocus.current = true
              }
            }}
          >
            <DropdownMenuRadioGroup
              value={value}
              onValueChange={(next) => {
                const option = options.find((item) => item.value === next)
                if (!disabled && option && !option.disabled && next !== value) onChange(next)
              }}
            >
              {options.map((option) => (
                <DropdownMenuRadioItem
                  key={option.value}
                  value={option.value}
                  disabled={disabled || option.disabled}
                >
                  {option.label}
                </DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
          </DropdownMenuContent>
        </DropdownMenu>
      </dd>
    </>
  )
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
    <dl
      className="grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)] items-center gap-x-3 gap-y-1 text-sm"
      aria-label={t('issue.details')}
    >
      <MetaChoice
        label={t('issue.status')}
        value={issue.status_id}
        fallback={statusLabel(issue.status, t)}
        disabled={disabled}
        onChange={(status_id) => onChange({ status_id })}
        options={statusOptions.map((status) => ({
          value: status.id,
          label: statusLabel(status.name, t),
        }))}
      />
      <MetaChoice
        label={t('issue.priority')}
        value={issue.priority}
        disabled={disabled}
        onChange={(priority) => onChange({ priority })}
        options={priorities.map((priority) => ({
          value: priority,
          label: t(`priority.${priority.toLowerCase()}`, { defaultValue: priority }),
        }))}
      />
      <MetaChoice
        label={t('issue.assignee')}
        value={issue.assignee_id ?? ''}
        disabled={disabled || (!usersQuery.data && (usersQuery.isLoading || usersQuery.isError))}
        onChange={(assignee_id) => onChange({ assignee_id: assignee_id || null })}
        options={assigneeOptions}
      />
      {usersQuery.isError && (
        <dd className="col-span-2">
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
        </dd>
      )}
      {sprints && (
        <MetaChoice
          label={t('issue.sprint')}
          value={issue.sprint_id ?? ''}
          disabled={disabled}
          onChange={(sprint_id) => onChange({ sprint_id: sprint_id || null })}
          options={[
            { value: '', label: t('issue.noSprint') },
            ...sprints.map((sprint) => ({ value: sprint.id, label: sprint.name })),
          ]}
        />
      )}
    </dl>
  )
}
