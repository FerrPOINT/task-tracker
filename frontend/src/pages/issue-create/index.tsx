import { useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router'
import { Plus } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { ErrorState } from '@/shared/ui/async-states'
import { Input } from '@/shared/ui/input'
import { useCreateIssue, useIssueTypes, useProjects, useUsers } from '@/shared/api/hooks'
import { useAuthStore } from '@/shared/auth/store'

export function IssueCreatePage() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { mutate, isPending, error } = useCreateIssue()
  const userId = useAuthStore((s) => s.userId)
  const projectsQuery = useProjects()
  const usersQuery = useUsers()
  const issueTypesQuery = useIssueTypes()

  const [project_key, setProjectKey] = useState('')
  const [type, setType] = useState('Task')
  const [summary, setSummary] = useState('')
  const [description, setDescription] = useState('')
  const [priority, setPriority] = useState('Medium')
  const [assignee_id, setAssigneeId] = useState('')

  const projects = useMemo(() => projectsQuery.data ?? [], [projectsQuery.data])
  const users = usersQuery.data ?? []
  const issueTypes = issueTypesQuery.data ?? []

  useEffect(() => {
    if (!project_key && projects.length > 0) {
      setProjectKey(projects[0]!.key)
    }
  }, [projects, project_key])

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!userId) {
      return
    }
    mutate(
      {
        project_key,
        issue_type: type.toLowerCase() as 'task' | 'story' | 'bug' | 'epic',
        summary,
        description: description || null,
        priority: priority.toLowerCase() as 'highest' | 'high' | 'medium' | 'low' | 'lowest',
        status_id: '00000000-0000-0000-0000-000000000001',
        assignee_id: assignee_id || null,
        reporter_id: userId,
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
              disabled={usersQuery.isLoading}
            >
              <option value="">{t('issueCreate.unassigned')}</option>
              {users.map((u) => (
                <option key={u.id} value={u.id}>
                  {u.display_name || u.username || u.email}
                </option>
              ))}
            </select>
          </div>
        </div>

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
