import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/shared/ui/dialog'
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
    const payload = isEdit
      ? ({ name, description: description || null } as UpdateProjectRequest)
      : ({ key: key.toUpperCase(), name, description: description || null } as CreateProjectRequest)
    onSubmit(payload)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {isEdit ? t('projects.editProject') : t('projects.createProject')}
          </DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4 pt-2">
          {error && <div className="text-sm text-rose-500">{error.message}</div>}
          <div className="space-y-2">
            <label className="text-sm font-medium">{t('projects.key')} *</label>
            <Input
              value={key}
              onChange={(e) => setKey(e.target.value.toUpperCase())}
              placeholder={t('projects.keyPlaceholder')}
              disabled={isEdit}
              required
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">{t('projects.name')} *</label>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('projects.namePlaceholder')}
              required
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">{t('projects.description')}</label>
            <textarea
              className="min-h-[100px] w-full rounded-md border border-border-strong bg-background p-3 text-sm text-text-primary"
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
        </form>
      </DialogContent>
    </Dialog>
  )
}
