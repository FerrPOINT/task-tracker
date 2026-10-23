import { Fieldset,
  ProjectAvatar,
  Button,
  Input,
  ErrorState,
  Textarea,
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
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
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!isPending) onOpenChange(next)
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            <span className="flex items-center gap-2">
              {isEdit && <ProjectAvatar projectKey={project.key} />}
              {isEdit ? t('projects.editProject') : t('projects.createProject')}
            </span>
          </DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4 pt-2">
          <Fieldset
            disabled={isPending}
            className="min-w-0 space-y-4"
            aria-busy={isPending || undefined}
          >
            {error && <ErrorState message={error.message} />}
            <div className="space-y-2">
              <label htmlFor="project-form-key" className="text-sm font-medium">
                {t('projects.key')} *
              </label>
              <Input
                id="project-form-key"
                value={key}
                onChange={(e) => setKey(e.target.value)}
                pattern="[A-Za-z0-9\x2D]{1,10}"
                maxLength={10}
                placeholder={t('projects.keyPlaceholder')}
                disabled={isEdit}
                required
              />
            </div>
            <div className="space-y-2">
              <label htmlFor="project-form-name" className="text-sm font-medium">
                {t('projects.name')} *
              </label>
              <Input
                id="project-form-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t('projects.namePlaceholder')}
                required
              />
            </div>
            <div className="space-y-2">
              <label htmlFor="project-form-description" className="text-sm font-medium">
                {t('projects.description')}
              </label>
              <Textarea
                id="project-form-description"
                className="min-h-[100px]"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder={t('projects.descriptionPlaceholder')}
              />
            </div>
            <div className="flex justify-end gap-2 pt-2">
              <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
                {t('common.cancel')}
              </Button>
              <Button type="submit" disabled={isPending}>
                {isPending ? t('common.saving') : isEdit ? t('common.save') : t('projects.create')}
              </Button>
            </div>
          </Fieldset>
        </form>
      </DialogContent>
    </Dialog>
  )
}
