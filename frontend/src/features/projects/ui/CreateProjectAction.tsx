import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@pdlc/ui/ui'
import { useCreateProject } from '@/shared/api/hooks'
import type { CreateProjectRequest, Project } from '@/api/project'
import { ProjectFormDialog } from './ProjectFormDialog'
import { CreateProjectInSpacesDialog } from './CreateProjectInSpacesDialog'

export function CreateProjectAction({ onCreated }: { onCreated?: (project: Project) => void }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const create = useCreateProject({ navigateOnSuccess: !onCreated })
  return (
    <>
      <Button
        type="button"
        onClick={() => {
          create.reset()
          setOpen(true)
        }}
      >
        {t('projects.createProject')}
      </Button>
      {import.meta.env.VITE_SPACE_ACCESS_ENABLED === 'true' ? (
        <CreateProjectInSpacesDialog
          open={open}
          onOpenChange={setOpen}
          onCreated={onCreated}
          navigateOnSuccess={!onCreated}
        />
      ) : (
        <ProjectFormDialog
          open={open}
          onOpenChange={setOpen}
          isPending={create.isPending}
          error={create.error}
          onSubmit={(values) =>
            create.mutate(values as CreateProjectRequest, {
              onSuccess: (project) => {
                setOpen(false)
                onCreated?.(project)
              },
            })
          }
        />
      )}
    </>
  )
}
