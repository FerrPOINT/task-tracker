import { useTranslation } from 'react-i18next'
import { useMemo } from 'react'
import { Button, Notice, ResourceListDescription, Select } from '@pdlc/ui/ui'
import type { Issue } from '@/api/issue'
import type { Sprint } from '@/api/sprint'
import type { components } from '@/api/generated'
import { useUsers } from '@/shared/api/hooks'

const priorities = ['Lowest', 'Low', 'Medium', 'High', 'Highest']

interface IssueMetaEditorProps {
  issue: Issue
  lifecycle?: components['schemas']['SdlcBoardTask']
  sprints?: Sprint[]
  onChange: (patch: {
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
  const choices = options.some((option) => option.value === value)
    ? options
    : [{ value, label: fallback ?? value, disabled: true }, ...options]
  const selected = choices.find((option) => option.value === value)?.label ?? value
  return (
    <div className="min-w-0">
      <dt>
        <ResourceListDescription>{label}</ResourceListDescription>
      </dt>
      <dd className="min-w-0">
        <Select
          value={value}
          options={choices}
          disabled={disabled}
          onValueChange={onChange}
          aria-label={`${label}: ${selected}`}
          className="w-full"
        />
      </dd>
    </div>
  )
}

export function IssueMetaEditor({
  issue,
  lifecycle,
  sprints,
  onChange,
  disabled,
}: IssueMetaEditorProps) {
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
    <dl
      className="grid grid-cols-2 items-start gap-x-3 gap-y-1 text-sm"
      aria-label={t('issue.details')}
    >
      {lifecycle && (
        <div className="min-w-0">
          <dt><ResourceListDescription>{t('issue.status')}</ResourceListDescription></dt>
          <dd>
            {t(`board.sdlcStage.${lifecycle.stage}`)}
            {lifecycle.status && ` · ${t(`board.sdlcStatus.${lifecycle.status}`)}`}
          </dd>
        </div>
      )}
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
