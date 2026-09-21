import { useEffect, useMemo, useState } from 'react'
import { useLocation, useNavigate, useSearchParams } from 'react-router'
import { Plus } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@sdlc/ui/ui'
import { ErrorState } from '@sdlc/ui/ui'
import { Input } from '@sdlc/ui/ui'
import {
  useCreateIssue,
  useIssueTypes,
  useProjectCustomFields,
  useProjectMembers,
  useProjects,
  useUsers,
} from '@/shared/api/hooks'
import { useAuthStore } from '@/shared/auth/store'
import {
  CustomFieldValueInput,
  isEmptyCustomFieldValue,
} from '@/features/issue-detail/ui/CustomFieldsPanel'

export function IssueCreatePage() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const location = useLocation()
  const [searchParams] = useSearchParams()
  const { mutate, isPending, error } = useCreateIssue()
  const userId = useAuthStore((s) => s.userId)
  const projectsQuery = useProjects()
  const usersQuery = useUsers()
  const issueTypesQuery = useIssueTypes()

  // Prefer ?project_key=..., then router state (board "+ Создать"), else first project.
  const [project_key, setProjectKey] = useState(
    () =>
      searchParams.get('project_key') ??
      (location.state as { project_key?: string } | null)?.project_key ??
      '',
  )
  const [type, setType] = useState('Task')
  const [summary, setSummary] = useState('')
  const [description, setDescription] = useState('')
  const [priority, setPriority] = useState('Medium')
  const [assignee_id, setAssigneeId] = useState('')
  const [customFieldValues, setCustomFieldValues] = useState<Record<string, unknown>>({})
  const [validationError, setValidationError] = useState<string | null>(null)

  const projects = useMemo(() => projectsQuery.data ?? [], [projectsQuery.data])
  const issueTypes = useMemo(
    () => (issueTypesQuery.data ?? []).filter((issueType) => !issueType.is_subtask),
    [issueTypesQuery.data],
  )
  const currentProject = useMemo(
    () => projects.find((project) => project.key === project_key),
    [projects, project_key],
  )
  const selectedProjectKey = currentProject?.key ?? ''
  const projectMembersQuery = useProjectMembers(selectedProjectKey)
  const customFieldsQuery = useProjectCustomFields(selectedProjectKey || undefined)
  const customFields = customFieldsQuery.data ?? []
  const selectedType = issueTypes.some((issueType) => issueType.name === type)
    ? type
    : (issueTypes[0]?.name ?? '')
  const setupLoading =
    projectsQuery.isLoading ||
    issueTypesQuery.isLoading ||
    (Boolean(currentProject) && customFieldsQuery.isLoading)
  const setupError =
    Boolean(projectsQuery.error) ||
    Boolean(issueTypesQuery.error) ||
    Boolean(customFieldsQuery.error)
  const canSubmit =
    !isPending &&
    !setupLoading &&
    !setupError &&
    Boolean(userId && currentProject && selectedType && summary.trim())
  const assignableUsers = useMemo(() => {
    const allowedIds = new Set((projectMembersQuery.data?.members ?? []).map((m) => m.user_id))
    if (currentProject?.owner_id) {
      allowedIds.add(currentProject.owner_id)
    }
    const users = usersQuery.data ?? []
    return users.filter((user) => allowedIds.has(user.id))
  }, [currentProject?.owner_id, projectMembersQuery.data?.members, usersQuery.data])

  useEffect(() => {
    if (!project_key && projects.length > 0) {
      setProjectKey(projects[0]!.key)
    }
  }, [projects, project_key])

  useEffect(() => {
    setCustomFieldValues({})
    setValidationError(null)
  }, [project_key])

  useEffect(() => {
    if (selectedType && selectedType !== type) setType(selectedType)
  }, [selectedType, type])

  useEffect(() => {
    if (assignee_id && !assignableUsers.some((user) => user.id === assignee_id)) {
      setAssigneeId('')
    }
  }, [assignableUsers, assignee_id])

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!canSubmit) return
    const missingRequired = customFields.find(
      (field) => field.is_required && isEmptyCustomFieldValue(customFieldValues[field.id]),
    )
    if (missingRequired) {
      setValidationError(t('customFields.requiredMissing', { name: missingRequired.name }))
      return
    }
    setValidationError(null)
    const custom_fields = Object.fromEntries(
      customFields
        .map((field) => [field.id, customFieldValues[field.id]] as const)
        .filter(([, value]) => !isEmptyCustomFieldValue(value)),
    )
    mutate(
      {
        project_key: selectedProjectKey,
        issue_type: selectedType.toLowerCase(),
        summary: summary.trim(),
        description: description || null,
        priority: priority.toLowerCase(),
        assignee_id: assignee_id || null,
        custom_fields,
      },
      {
        onSuccess: () => navigate(`/projects/${selectedProjectKey}/backlog`),
      },
    )
  }

  return (
    <div>
      <h1 className="mb-5 text-xl font-bold sm:text-2xl">{t('issueCreate.title')}</h1>

      <form onSubmit={handleSubmit} className="max-w-4xl space-y-5 border-t border-border pt-5">
        {error && <ErrorState message={t('issueCreate.saveError')} />}
        {validationError && <ErrorState message={validationError} />}
        {!userId && <div className="text-sm text-danger">{t('issueCreate.noReporter')}</div>}
        {setupLoading && <p className="text-sm text-text-muted">{t('issueCreate.loading')}</p>}
        {projectsQuery.error && (
          <ErrorState
            message={t('issueCreate.projectsError')}
            onRetry={() => void projectsQuery.refetch()}
          />
        )}
        {!projectsQuery.isLoading && !projectsQuery.error && projects.length === 0 && (
          <ErrorState message={t('issueCreate.noProjects')} />
        )}
        {!projectsQuery.isLoading && !projectsQuery.error && project_key && !currentProject && (
          <ErrorState message={t('issueCreate.projectUnavailable')} />
        )}
        {issueTypesQuery.error && (
          <ErrorState
            message={t('issueCreate.typesError')}
            onRetry={() => void issueTypesQuery.refetch()}
          />
        )}
        {!issueTypesQuery.isLoading && !issueTypesQuery.error && issueTypes.length === 0 && (
          <ErrorState message={t('issueCreate.noTypes')} />
        )}
        {customFieldsQuery.error && (
          <ErrorState
            message={t('issueCreate.fieldsError')}
            onRetry={() => void customFieldsQuery.refetch()}
          />
        )}
        {(usersQuery.error || projectMembersQuery.error) && currentProject && (
          <ErrorState
            message={t('issueCreate.assigneesError')}
            onRetry={() => {
              if (usersQuery.error) void usersQuery.refetch()
              if (projectMembersQuery.error) void projectMembersQuery.refetch()
            }}
          />
        )}

        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <label htmlFor="issue-project" className="text-sm font-medium">
              {t('issueCreate.project')} *
            </label>
            <select
              id="issue-project"
              className="h-11 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary sm:h-10"
              value={selectedProjectKey}
              onChange={(e) => setProjectKey(e.target.value)}
              disabled={projectsQuery.isLoading || projects.length === 0 || isPending}
              required
            >
              {!currentProject && (
                <option value="" disabled>
                  {t('issueCreate.selectProject')}
                </option>
              )}
              {projects.map((p) => (
                <option key={p.key} value={p.key}>
                  {p.name} ({p.key})
                </option>
              ))}
            </select>
          </div>
          <div className="space-y-2">
            <label htmlFor="issue-type" className="text-sm font-medium">
              {t('issueCreate.type')} *
            </label>
            <select
              id="issue-type"
              className="h-11 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary sm:h-10"
              value={selectedType}
              onChange={(e) => setType(e.target.value)}
              disabled={issueTypesQuery.isLoading || issueTypes.length === 0 || isPending}
              required
            >
              {issueTypes.map((issueType) => (
                <option key={issueType.id} value={issueType.name}>
                  {t(`issueType.${issueType.name.toLowerCase()}`, {
                    defaultValue: issueType.name,
                  })}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="space-y-2">
          <label htmlFor="issue-summary" className="text-sm font-medium">
            {t('issueCreate.summary')} *
          </label>
          <Input
            id="issue-summary"
            type="text"
            placeholder={t('issueCreate.summaryPlaceholder')}
            value={summary}
            onChange={(e) => {
              setSummary(e.target.value)
              setValidationError(null)
            }}
            required
            className="min-h-11 sm:min-h-10"
            disabled={isPending}
          />
        </div>

        <div className="space-y-2">
          <label htmlFor="issue-description" className="text-sm font-medium">
            {t('issueCreate.description')}
          </label>
          <textarea
            id="issue-description"
            className="min-h-[120px] w-full rounded-md border border-border-strong bg-background p-3 text-sm text-text-primary"
            placeholder={t('issueCreate.descriptionPlaceholder')}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            disabled={isPending}
          />
        </div>

        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <label htmlFor="issue-priority" className="text-sm font-medium">
              {t('issueCreate.priority')}
            </label>
            <select
              id="issue-priority"
              className="h-11 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary sm:h-10"
              value={priority}
              onChange={(e) => setPriority(e.target.value)}
              disabled={isPending}
            >
              <option value="Medium">{t('priority.medium')}</option>
              <option value="Highest">{t('priority.highest')}</option>
              <option value="High">{t('priority.high')}</option>
              <option value="Low">{t('priority.low')}</option>
              <option value="Lowest">{t('priority.lowest')}</option>
            </select>
          </div>
          <div className="space-y-2">
            <label htmlFor="issue-assignee" className="text-sm font-medium">
              {t('issueCreate.assignee')}
            </label>
            <select
              id="issue-assignee"
              className="h-11 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary sm:h-10"
              value={assignee_id}
              onChange={(e) => setAssigneeId(e.target.value)}
              disabled={
                isPending ||
                usersQuery.isLoading ||
                projectMembersQuery.isLoading ||
                Boolean(usersQuery.error || projectMembersQuery.error)
              }
            >
              <option value="">{t('issueCreate.unassigned')}</option>
              {assignableUsers.map((u) => (
                <option key={u.id} value={u.id}>
                  {u.display_name || u.username}
                </option>
              ))}
            </select>
          </div>
        </div>

        {customFields.length > 0 && (
          <fieldset disabled={isPending} className="space-y-3 border-t border-border pt-4">
            <h2 className="text-sm font-semibold">{t('customFields.title')}</h2>
            <div className="grid gap-4 sm:grid-cols-2">
              {customFields.map((field) => (
                <label
                  key={field.id}
                  className="space-y-2 text-sm font-medium [&_input]:min-h-11 [&_select]:min-h-11 sm:[&_input]:min-h-10 sm:[&_select]:min-h-10"
                >
                  <span>
                    {field.name}
                    {field.is_required ? ' *' : ''}
                  </span>
                  <CustomFieldValueInput
                    field={field}
                    value={customFieldValues[field.id]}
                    onSave={(value) => {
                      setCustomFieldValues((prev) => ({ ...prev, [field.id]: value }))
                      setValidationError(null)
                    }}
                    commit="change"
                  />
                </label>
              ))}
            </div>
          </fieldset>
        )}

        <div className="flex gap-2 pt-2">
          <Button type="submit" disabled={!canSubmit} className="min-h-11 gap-1 sm:min-h-10">
            <Plus className="h-4 w-4" />
            {isPending ? t('common.creating') : t('issueCreate.submit')}
          </Button>
          <Button
            type="button"
            variant="outline"
            className="min-h-11 sm:min-h-10"
            disabled={isPending}
            onClick={() => navigate(-1)}
          >
            {t('common.cancel')}
          </Button>
        </div>
      </form>
    </div>
  )
}
