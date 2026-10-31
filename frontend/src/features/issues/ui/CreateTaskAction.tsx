import type { ComponentProps } from 'react'
import { Button, toast } from '@pdlc/ui/ui'
import { Plus } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useProjectNavigate } from '@/shared/lib/project-navigation'

type Props = Omit<ComponentProps<typeof Button>, 'asChild' | 'onClick'> & {
  projectKey?: string
  contextPending?: boolean
  contextError?: string
}

/** Ordinary creation has no Fleet, Workflow, model or runner prerequisite. */
export function CreateTaskAction({
  projectKey,
  contextPending = false,
  contextError,
  children,
  disabled,
  ...props
}: Props) {
  const { t } = useTranslation()
  const navigate = useProjectNavigate()
  return (
    <Button
      {...props}
      type="button"
      disabled={disabled || contextPending}
      aria-label={props['aria-label'] ?? t('navigation.create')}
      aria-busy={contextPending}
      onClick={() => {
        if (contextError) {
          toast.error(contextError)
          return
        }
        const query = new URLSearchParams({ source: 'navigation' })
        if (projectKey) query.set('project_key', projectKey)
        void navigate(`/issues/create?${query}`)
      }}
    >
      <Plus className="h-4 w-4" aria-hidden />
      {children ?? t('navigation.create')}
    </Button>
  )
}
