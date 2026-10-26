import { useId, useState } from 'react'
import { Outlet, useOutletContext, useParams, useLocation } from 'react-router'
import { useTranslation } from 'react-i18next'
import {
  PageHeader,
  SectionNavigation,
  SectionNavigationItem,
  SectionHeader,
  toast,
  Textarea,
  Button,
  ErrorState,
  Input,
  FormField,
  LoadingState,
  ProjectAvatar,
  Notice,
} from '@pdlc/ui/ui'

import { useProjectDetails, useUpdateProject } from '@/shared/api/hooks'
import { ProjectLink } from '@/shared/lib/project-navigation'
import type { Project } from '@/api/project'
import { ProjectSpacesSettings } from '@/features/projects/ui/ProjectSpacesSettings'

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
      <PageHeader
        title={`${project.data.name} ${projectKey}`}
        leading={<ProjectAvatar projectKey={projectKey} />}
        context={t('projectSettings.title')}
      />
      <SectionNavigation aria-label={t('projectSettings.title')}>
        {[
          { to: base, label: t('projectSettings.general') },
          { to: `${base}/custom-fields`, label: t('customFields.title') },
        ].map(({ to, label }) => (
          <SectionNavigationItem key={to} asChild active={pathname === to}>
            <ProjectLink to={to}>{label}</ProjectLink>
          </SectionNavigationItem>
        ))}
      </SectionNavigation>
      <Outlet key={project.data.id} context={project.data} />
    </div>
  )
}

export function ProjectGeneralSettingsPage() {
  const project = useOutletContext<Project>()
  // A different project gets a fresh draft; background refetches cannot erase unsaved input.
  return (
    <div className="space-y-6">
      <ProjectPropertiesForm key={project.id} project={project} />
      <ProjectSpacesSettings project={project} />
    </div>
  )
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
      <SectionHeader level={2} title={<>{t('projectSettings.general')}</>} />
      <FormField
        id={`${id}-key`}
        label={t('projects.key')}
        hint={<span id={`${id}-key-help`}>{t('projectSettings.keyHelp')}</span>}
      >
        {(attrs) => (
          <Input {...attrs} value={project.key} readOnly aria-describedby={`${id}-key-help`} />
        )}
      </FormField>
      <FormField id={`${id}-name`} label={t('projects.name')} required>
        {(attrs) => (
          <Input
            {...attrs}
            value={name}
            required
            disabled={update.isPending}
            onChange={(event) => {
              update.reset()
              setName(event.target.value)
            }}
          />
        )}
      </FormField>
      <FormField id={`${id}-description`} label={t('projects.description')}>
        {(attrs) => (
          <Textarea
            {...attrs}
            className="w-full min-h-28"
            value={description}
            disabled={update.isPending}
            onChange={(event) => {
              update.reset()
              setDescription(event.target.value)
            }}
          />
        )}
      </FormField>
      {update.isError && (
        <Notice role="alert" tone="danger" appearance="inline">
          {t('projectSettings.saveError')}
        </Notice>
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
