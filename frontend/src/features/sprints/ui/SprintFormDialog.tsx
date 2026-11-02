import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { FormDialog, FormField, Notice, Textarea, Button, Input } from '@pdlc/ui/ui'

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
    <FormDialog
      open={open}
      onOpenChange={onOpenChange}
      pending={isPending}
      title={<>{isEdit ? t('sprints.editSprint') : t('sprints.createSprint')}</>}
      onSubmit={handleSubmit}
      footer={
        <>
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={isPending}>
            {isPending ? t('common.saving') : isEdit ? t('common.save') : t('sprints.create')}
          </Button>
        </>
      }
    >
      {error && (
        <Notice role="alert" tone="danger" appearance="inline">
          {error.message}
        </Notice>
      )}
      <FormField id="sprint-form-name" label={t('sprints.name')} required={!isEdit}>
        {(attrs) => (
          <Input
            {...attrs}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t('sprints.namePlaceholder')}
          />
        )}
      </FormField>
      <FormField id="sprint-form-goal" label={t('sprints.goal')}>
        {(attrs) => (
          <Textarea
            {...attrs}
            className="w-full"
            value={goal}
            onChange={(e) => setGoal(e.target.value)}
            placeholder={t('sprints.goalPlaceholder')}
          />
        )}
      </FormField>
      <div className="grid gap-3 sm:grid-cols-2">
        <FormField id="sprint-form-startdate" label={t('sprints.startDate')}>
          {(attrs) => (
            <Input
              {...attrs}
              type="date"
              value={startDate}
              onChange={(e) => setStartDate(e.target.value)}
            />
          )}
        </FormField>
        <FormField
          id="sprint-form-enddate"
          label={t('sprints.endDate')}
          error={dateError ?? undefined}
        >
          {(attrs) => (
            <Input
              {...attrs}
              type="date"
              value={endDate}
              onChange={(e) => setEndDate(e.target.value)}
            />
          )}
        </FormField>
      </div>
    </FormDialog>
  )
}
