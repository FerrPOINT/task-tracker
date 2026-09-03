import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@sdlc/ui/ui'
import { Input } from '@sdlc/ui/ui'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@sdlc/ui/ui'
import type { Sprint, CreateSprintRequest, UpdateSprintRequest } from '@/api/sprint'

interface SprintFormDialogProps {
  open: boolean
  sprint?: Sprint | null
  onOpenChange: (open: boolean) => void
  onSubmit: (values: CreateSprintRequest | UpdateSprintRequest) => void
  isPending: boolean
  error?: Error | null
}

export function SprintFormDialog({
  open,
  sprint,
  onOpenChange,
  onSubmit,
  isPending,
  error,
}: SprintFormDialogProps) {
  const { t } = useTranslation()
  const [name, setName] = useState('')
  const [goal, setGoal] = useState('')
  const [startDate, setStartDate] = useState('')
  const [endDate, setEndDate] = useState('')
  const [dateError, setDateError] = useState<string | null>(null)
  const isEdit = !!sprint

  useEffect(() => {
    if (open) {
      setName(sprint?.name ?? '')
      setGoal(sprint?.goal ?? '')
      setStartDate(sprint?.start_date ? sprint.start_date.slice(0, 10) : '')
      setEndDate(sprint?.end_date ? sprint.end_date.slice(0, 10) : '')
    }
  }, [open, sprint])

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    // Server rejects inverted ranges; fail fast in the dialog as well.
    if (startDate && endDate && endDate < startDate) {
      setDateError(t('sprints.endBeforeStart', 'Дата окончания не может быть раньше начала'))
      return
    }
    setDateError(null)
    const payload = isEdit
      ? ({
          name: name || undefined,
          goal: goal === '' ? null : goal,
          start_date: startDate ? `${startDate}T00:00:00+00:00` : null,
          end_date: endDate ? `${endDate}T00:00:00+00:00` : null,
        } as UpdateSprintRequest)
      : ({
          name,
          goal: goal || undefined,
          start_date: startDate ? `${startDate}T00:00:00+00:00` : undefined,
          end_date: endDate ? `${endDate}T00:00:00+00:00` : undefined,
        } as CreateSprintRequest)
    onSubmit(payload)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{isEdit ? t('sprints.editSprint') : t('sprints.createSprint')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4 pt-2">
          {error && <div className="text-sm text-rose-500">{error.message}</div>}
          <div className="space-y-2">
            <label htmlFor="sprint-form-name" className="text-sm font-medium">
              {t('sprints.name')} *
            </label>
            <Input
              id="sprint-form-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('sprints.namePlaceholder')}
              required={!isEdit}
            />
          </div>
          <div className="space-y-2">
            <label htmlFor="sprint-form-goal" className="text-sm font-medium">
              {t('sprints.goal')}
            </label>
            <textarea
              className="min-h-[80px] w-full rounded-md border border-border-strong bg-background p-3 text-sm text-text-primary"
              value={goal}
              onChange={(e) => setGoal(e.target.value)}
              placeholder={t('sprints.goalPlaceholder')}
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-2">
              <label htmlFor="sprint-form-startdate" className="text-sm font-medium">
                {t('sprints.startDate')}
              </label>
              <Input type="date" value={startDate} onChange={(e) => setStartDate(e.target.value)} />
            </div>
            <div className="space-y-2">
              <label htmlFor="sprint-form-enddate" className="text-sm font-medium">
                {t('sprints.endDate')}
              </label>
              <Input type="date" value={endDate} onChange={(e) => setEndDate(e.target.value)} />
            </div>
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </Button>
            {dateError && <p className="text-sm text-destructive">{dateError}</p>}
            <Button type="submit" disabled={isPending}>
              {isPending ? t('common.saving') : isEdit ? t('common.save') : t('sprints.create')}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
