import { useEffect, useState } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useTranslation } from 'react-i18next'
import { Play, Square } from 'lucide-react'
import { FormDialog, Button, Input, Notice, FormField } from '@pdlc/ui/ui'

import { parseDuration, formatDuration } from '@/shared/lib/time'
import type { Worklog, LogWorkInput } from '@/entities/worklog/model'

interface LogWorkDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubmit: (input: LogWorkInput) => void
  worklog?: Worklog
  isPending?: boolean
  error?: Error | null
}

export function LogWorkDialog({
  open,
  onOpenChange,
  onSubmit,
  worklog,
  isPending = false,
  error,
}: LogWorkDialogProps) {
  const { t } = useTranslation()
  const isEdit = Boolean(worklog)
  const [timerRunning, setTimerRunning] = useState(false)
  const [timerSeconds, setTimerSeconds] = useState(0)

  const schema = z.object({
    timeSpent: z
      .string()
      .min(1, t('timeTracking.validation.timeSpentRequired'))
      .refine(
        (v) => {
          const parsed = parseDuration(v)
          return parsed !== null && parsed > 0
        },
        {
          message: t('timeTracking.validation.invalidFormat'),
        },
      ),
    startedAt: z.string().min(1),
    comment: z.string(),
  })

  const form = useForm<{
    timeSpent: string
    startedAt: string
    comment: string
  }>({
    resolver: zodResolver(schema),
    defaultValues: {
      timeSpent: '',
      startedAt: new Date().toISOString().slice(0, 10),
      comment: '',
    },
  })

  useEffect(() => {
    if (open) {
      const wl = worklog
      form.reset({
        timeSpent: wl ? formatDuration(wl.timeSpentSeconds) : '',
        startedAt: wl
          ? new Date(wl.startedAt).toISOString().slice(0, 10)
          : new Date().toISOString().slice(0, 10),
        comment: wl?.comment ?? '',
      })
    } else {
      setTimerRunning(false)
      setTimerSeconds(0)
    }
  }, [open, worklog, form])

  useEffect(() => {
    if (!timerRunning) return
    const interval = setInterval(() => setTimerSeconds((s) => s + 1), 1000)
    return () => clearInterval(interval)
  }, [timerRunning])

  const toggleTimer = () => {
    if (timerRunning) {
      setTimerRunning(false)
      const current = parseDuration(form.getValues('timeSpent')) ?? 0
      form.setValue('timeSpent', formatDuration(current + timerSeconds))
      setTimerSeconds(0)
    } else {
      setTimerRunning(true)
    }
  }

  const handleSubmit = form.handleSubmit((values) => {
    if (isPending) return
    onSubmit({
      timeSpent: values.timeSpent,
      startedAt: new Date(values.startedAt).toISOString(),
      comment: values.comment,
    })
  })

  const handleOpenChange = (nextOpen: boolean) => {
    if (isPending) return
    if (!nextOpen) {
      setTimerRunning(false)
      setTimerSeconds(0)
    }
    onOpenChange(nextOpen)
  }

  return (
    <FormDialog
      open={open}
      onOpenChange={handleOpenChange}
      pending={isPending}
      title={<>{isEdit ? t('timeTracking.editWorklog') : t('timeTracking.logWork')}</>}
      onSubmit={(event) => void handleSubmit(event)}
      footer={
        <>
          <Button
            type="button"
            variant="secondary"
            onClick={() => handleOpenChange(false)}
            disabled={isPending}
          >
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={isPending}>
            {isPending ? t('common.saving') : t('common.save')}
          </Button>
        </>
      }
    >
      {error && (
        <Notice role="alert" tone="danger" appearance="inline">
          {error.message}
        </Notice>
      )}
      <FormField
        id="timeSpent"
        label={t('timeTracking.fields.timeSpent')}
        error={form.formState.errors.timeSpent?.message}
        hint={timerRunning ? formatDuration(timerSeconds) : undefined}
      >
        {(attrs) => (
          <div className="flex gap-2">
            <Input {...attrs} {...form.register('timeSpent')} placeholder="1h 30m" />
            <Button
              type="button"
              variant="secondary"
              size="icon"
              onClick={toggleTimer}
              aria-label={
                timerRunning ? t('timeTracking.timer.stop') : t('timeTracking.timer.start')
              }
            >
              {timerRunning ? <Square className="h-4 w-4" /> : <Play className="h-4 w-4" />}
            </Button>
          </div>
        )}
      </FormField>

      <FormField id="startedAt" label={t('timeTracking.fields.startedAt')}>
        {(attrs) => <Input {...attrs} type="date" {...form.register('startedAt')} />}
      </FormField>

      <FormField id="comment" label={t('timeTracking.fields.comment')}>
        {(attrs) => (
          <Input
            {...attrs}
            {...form.register('comment')}
            placeholder={t('timeTracking.fields.commentPlaceholder')}
          />
        )}
      </FormField>
    </FormDialog>
  )
}
