import { useId, useState } from 'react'
import { Outlet, useOutletContext, useParams, useLocation } from 'react-router'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Textarea, Button, ErrorState, Input, LoadingState, ProjectAvatar } from '@pdlc/ui/ui'
import { useProjectDetails, useUpdateProject } from '@/shared/api/hooks'
import { ProjectLink } from '@/shared/lib/project-navigation'
import type { Project } from '@/api/project'

export function ProjectSettingsPage() {
  const { projectKey = '' } = useParams()
  const { pathname } = useLocation()
  const { t } = useTranslation()
  const project = useProjectDetails(projectKey)
  if (project.isPending) return <LoadingState />
  if (project.isError)
    return (
      <ErrorState message={t('projectSettings.loadError')} onRetry={() => void project.refetch()} />
    )
  const base = `/projects/${encodeURIComponent(projectKey)}/settings`
  return (
    <div className="space-y-6">
      <header className="space-y-3">
        <p className="text-sm text-text-muted">{t('projectSettings.title')}</p>
        <h1 className="flex min-w-0 items-center gap-3 text-xl font-semibold sm:text-2xl">
          <ProjectAvatar projectKey={projectKey} />
          <span className="break-words">{project.data.name}</span>
          <span className="text-sm font-normal text-text-muted">{projectKey}</span>
        </h1>
      </header>
      <nav
        aria-label={t('projectSettings.title')}
        className="flex flex-wrap gap-1 border-b border-border"
      >
        {[
          { to: base, label: t('projectSettings.general') },
          { to: `${base}/custom-fields`, label: t('customFields.title') },
        ].map(({ to, label }) => (
          <ProjectLink
            key={to}
            to={to}
            aria-current={pathname === to ? 'page' : undefined}
            className={`min-h-11 border-b-2 px-3 py-3 text-sm font-medium ${pathname === to ? 'border-accent text-accent' : 'border-transparent text-text-muted hover:text-text-primary'}`}
          >
            {label}
          </ProjectLink>
        ))}
      </nav>
      <Outlet key={project.data.id} context={project.data} />
    </div>
  )
}

export function ProjectGeneralSettingsPage() {
  const project = useOutletContext<Project>()
  // A different project gets a fresh draft; background refetches cannot erase unsaved input.
  return <ProjectPropertiesForm key={project.id} project={project} />
}

function ProjectPropertiesForm({ project }: { project: Project }) {
  const { t } = useTranslation()
  const id = useId()
  const [nameDraft, setName] = useState<string | null>(null)
  const [descriptionDraft, setDescription] = useState<string | null>(null)
  const name = nameDraft ?? project.name
  const description = descriptionDraft ?? project.description ?? ''
  const update = useUpdateProject(project.key)
  const nameChanged = nameDraft !== null && name !== project.name
  const descriptionChanged =
    descriptionDraft !== null && description !== (project.description ?? '')
  const dirty = nameChanged || descriptionChanged
  return (
    <form
      className="max-w-2xl space-y-5"
      onSubmit={(event) => {
        event.preventDefault()
        if (!name.trim() || !dirty || update.isPending) return
        update.mutate(
          {
            ...(nameChanged ? { name: name.trim() } : {}),
            ...(descriptionChanged ? { description: description.trim() || null } : {}),
          },
          {
            onSuccess: () => {
              setName(null)
              setDescription(null)
              toast.success(t('projectSettings.saved'))
            },
          },
        )
      }}
    >
      <h2 className="text-base font-semibold">{t('projectSettings.general')}</h2>
      <div className="space-y-2 text-sm">
        <label htmlFor={`${id}-key`}>{t('projects.key')}</label>
        <Input id={`${id}-key`} value={project.key} readOnly aria-describedby={`${id}-key-help`} />
        <p id={`${id}-key-help`} className="text-xs text-text-muted">
          {t('projectSettings.keyHelp')}
        </p>
      </div>
      <div className="space-y-2 text-sm">
        <label htmlFor={`${id}-name`}>{t('projects.name')}</label>
        <Input
          id={`${id}-name`}
          value={name}
          required
          disabled={update.isPending}
          onChange={(event) => {
            update.reset()
            setName(event.target.value)
          }}
        />
      </div>
      <div className="space-y-2 text-sm">
        <label htmlFor={`${id}-description`}>{t('projects.description')}</label>
        <Textarea
          id={`${id}-description`}
          className="w-full min-h-28"
          value={description}
          disabled={update.isPending}
          onChange={(event) => {
            update.reset()
            setDescription(event.target.value)
          }}
        />
      </div>
      {update.isError && (
        <p role="alert" className="text-sm text-danger">
          {t('projectSettings.saveError')}
        </p>
      )}
      <div className="flex gap-2">
        <Button type="submit" disabled={!dirty || !name.trim() || update.isPending}>
          {update.isPending ? t('common.saving') : t('common.save')}
        </Button>
        <Button
          type="button"
          variant="outline"
          disabled={!dirty || update.isPending}
          onClick={() => {
            setName(null)
            setDescription(null)
            update.reset()
          }}
        >
          {t('common.cancel')}
        </Button>
      </div>
    </form>
  )
}
