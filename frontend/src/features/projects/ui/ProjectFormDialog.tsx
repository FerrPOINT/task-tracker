import {
  FormDialog,
  ProjectAvatar,
  Button,
  Input,
  ErrorState,
  Textarea,
  FormField,
} from '@pdlc/ui/ui'
import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'

import type { Project, CreateProjectRequest, UpdateProjectRequest } from '@/api/project'

interface ProjectFormDialogProps {
  open: boolean
  project?: Project | null
  onOpenChange: (open: boolean) => void
  onSubmit: (values: CreateProjectRequest | UpdateProjectRequest) => void
  isPending: boolean
  error?: Error | null
}

export function ProjectFormDialog({
  open,
  project,
  onOpenChange,
  onSubmit,
  isPending,
  error,
}: ProjectFormDialogProps) {
  const { t } = useTranslation()
  const [key, setKey] = useState('')
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const isEdit = !!project

  useEffect(() => {
    if (open) {
      setKey(project?.key ?? '')
      setName(project?.name ?? '')
      setDescription(project?.description ?? '')
    }
  }, [open, project])

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    e.stopPropagation()
    if (isPending) return
    const payload = isEdit
      ? ({ name, description: description || null } as UpdateProjectRequest)
      : ({ key, name, description: description || null } as CreateProjectRequest)
    onSubmit(payload)
  }

  return (
    <FormDialog
      open={open}
      onOpenChange={(next) => {
        if (!isPending) onOpenChange(next)
      }}
      pending={isPending}
      title={
        <>
          <span className="flex items-center gap-2">
            {isEdit && <ProjectAvatar projectKey={project.key} />}
            {isEdit ? t('projects.editProject') : t('projects.createProject')}
          </span>
        </>
      }
      onSubmit={handleSubmit}
      footer={
        <>
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={isPending}>
            {isPending ? t('common.saving') : isEdit ? t('common.save') : t('projects.create')}
          </Button>
        </>
      }
    >
      {error && <ErrorState message={error.message} />}
      <FormField id="project-form-key" label={t('projects.key') + ' *'}>
        {(baseFieldAttrs) => (
          <Input
            {...baseFieldAttrs}
            value={key}
            onChange={(e) => setKey(e.target.value)}
            pattern="[A-Za-z0-9\x2D]{1,10}"
            maxLength={10}
            placeholder={t('projects.keyPlaceholder')}
            disabled={isEdit}
            required
          />
        )}
      </FormField>
      <FormField id="project-form-name" label={t('projects.name') + ' *'}>
        {(baseFieldAttrs) => (
          <Input
            {...baseFieldAttrs}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t('projects.namePlaceholder')}
            required
          />
        )}
      </FormField>
      <FormField id="project-form-description" label={t('projects.description')}>
        {(baseFieldAttrs) => (
          <Textarea
            {...baseFieldAttrs}
            className="min-h-[100px]"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder={t('projects.descriptionPlaceholder')}
          />
        )}
      </FormField>
    </FormDialog>
  )
}
