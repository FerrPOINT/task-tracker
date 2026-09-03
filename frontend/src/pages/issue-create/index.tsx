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
  const issueTypes = issueTypesQuery.data ?? []
  const projectMembersQuery = useProjectMembers(project_key)
  const customFieldsQuery = useProjectCustomFields(project_key || undefined)
  const customFields = customFieldsQuery.data ?? []
  const currentProject = useMemo(
    () => projects.find((project) => project.key === project_key),
    [projects, project_key],
  )
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
    if (assignee_id && !assignableUsers.some((user) => user.id === assignee_id)) {
      setAssigneeId('')
    }
  }, [assignableUsers, assignee_id])

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!userId) {
      return
    }
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
        project_key,
        issue_type: type.toLowerCase() as 'task' | 'story' | 'bug' | 'epic',
        summary,
        description: description || null,
        priority: priority.toLowerCase() as 'highest' | 'high' | 'medium' | 'low' | 'lowest',
        assignee_id: assignee_id || null,
        custom_fields,
      },
      {
        onSuccess: () => navigate(`/projects/${project_key}/backlog`),
      },
    )
  }

  return (
    <div>
      <h1 className="mb-5 text-xl font-bold sm:text-2xl">{t('issueCreate.title')}</h1>

      <form
        onSubmit={handleSubmit}
        className="space-y-4 rounded-lg border border-border bg-surface p-4 sm:p-6"
      >
        {error && <ErrorState message={error.message} />}
        {validationError && <ErrorState message={validationError} />}
        {!userId && <div className="text-sm text-amber-500">{t('issueCreate.noReporter')}</div>}

        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <label htmlFor="issue-project" className="text-sm font-medium">
              {t('issueCreate.project')} *
            </label>
            <select
              id="issue-project"
              className="h-10 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary"
              value={project_key}
              onChange={(e) => setProjectKey(e.target.value)}
              disabled={projectsQuery.isLoading || projects.length === 0}
            >
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
              className="h-10 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary"
              value={type}
              onChange={(e) => setType(e.target.value)}
            >
              {issueTypes.length > 0
                ? issueTypes
                    .filter((it) => !it.is_subtask)
                    .map((it) => (
                      <option key={it.id} value={it.name}>
                        {t(`issueType.${it.name.toLowerCase()}`, { defaultValue: it.name })}
                      </option>
                    ))
                : ['Task', 'Story', 'Bug', 'Epic'].map((name) => (
                    <option key={name} value={name}>
                      {name}
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
            onChange={(e) => setSummary(e.target.value)}
            required
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
          />
        </div>

        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <label htmlFor="issue-priority" className="text-sm font-medium">
              {t('issueCreate.priority')}
            </label>
            <select
              id="issue-priority"
              className="h-10 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary"
              value={priority}
              onChange={(e) => setPriority(e.target.value)}
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
              className="h-10 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary"
              value={assignee_id}
              onChange={(e) => setAssigneeId(e.target.value)}
              disabled={usersQuery.isLoading || projectMembersQuery.isLoading}
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
          <div className="space-y-3 border-t border-border pt-4">
            <h2 className="text-sm font-semibold">{t('customFields.title')}</h2>
            <div className="grid gap-4 sm:grid-cols-2">
              {customFields.map((field) => (
                <label key={field.id} className="space-y-2 text-sm font-medium">
                  <span>
                    {field.name}
                    {field.is_required ? ' *' : ''}
                  </span>
                  <CustomFieldValueInput
                    field={field}
                    value={customFieldValues[field.id]}
                    onSave={(value) =>
                      setCustomFieldValues((prev) => ({ ...prev, [field.id]: value }))
                    }
                    commit="change"
                  />
                </label>
              ))}
            </div>
          </div>
        )}

        <div className="space-y-2">
          <label htmlFor="issue-reporter" className="text-sm font-medium">
            {t('issueCreate.reporter')}
          </label>
          <Input id="issue-reporter" type="text" value={t('issueCreate.me')} disabled />
        </div>

        <div className="flex gap-2 pt-2">
          <Button type="submit" disabled={isPending || !userId} className="gap-1">
            <Plus className="h-4 w-4" />
            {isPending ? t('common.creating') : t('issueCreate.submit')}
          </Button>
          <Button type="button" variant="outline" onClick={() => navigate(-1)}>
            {t('common.cancel')}
          </Button>
        </div>
      </form>
    </div>
  )
}
