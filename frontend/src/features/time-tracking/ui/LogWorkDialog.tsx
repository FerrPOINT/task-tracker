import { useEffect, useState } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useTranslation } from 'react-i18next'
import { Play, Square } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/shared/ui/dialog'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { parseDuration, formatDuration } from '@/shared/lib/time'
import type { Worklog, LogWorkInput } from '@/entities/worklog/model'

interface LogWorkDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubmit: (input: LogWorkInput) => void
  worklog?: Worklog
}

export function LogWorkDialog({ open, onOpenChange, onSubmit, worklog }: LogWorkDialogProps) {
  const { t } = useTranslation()
  const isEdit = Boolean(worklog)
  const [timerRunning, setTimerRunning] = useState(false)
  const [timerSeconds, setTimerSeconds] = useState(0)

  const schema = z.object({
    timeSpent: z
      .string()
      .min(1, t('timeTracking.validation.timeSpentRequired'))
      .refine((v) => {
        const parsed = parseDuration(v)
        return parsed !== null && parsed > 0
      }, {
        message: t('timeTracking.validation.invalidFormat'),
      }),
    remainingEstimate: z.string(),
    startedAt: z.string().min(1),
    comment: z.string(),
  })

  const form = useForm<{
    timeSpent: string
    remainingEstimate: string
    startedAt: string
    comment: string
  }>({
    resolver: zodResolver(schema),
    defaultValues: {
      timeSpent: '',
      remainingEstimate: '',
      startedAt: new Date().toISOString().slice(0, 10),
      comment: '',
    },
  })

  useEffect(() => {
    if (open) {
      const wl = worklog
      form.reset({
        timeSpent: wl ? formatDuration(wl.timeSpentSeconds) : '',
        remainingEstimate:
          wl?.remainingEstimateSeconds !== null && wl?.remainingEstimateSeconds !== undefined
            ? formatDuration(wl.remainingEstimateSeconds)
            : '',
        startedAt: wl
          ? new Date(wl.startedAt).toISOString().slice(0, 10)
          : new Date().toISOString().slice(0, 10),
        comment: wl?.comment ?? '',
      })
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
    onSubmit({
      timeSpent: values.timeSpent,
      remainingEstimate: values.remainingEstimate,
      startedAt: new Date(values.startedAt).toISOString(),
      comment: values.comment,
    })
    onOpenChange(false)
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{isEdit ? t('timeTracking.editWorklog') : t('timeTracking.logWork')}</DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-1">
            <Label htmlFor="timeSpent">{t('timeTracking.fields.timeSpent')}</Label>
            <div className="flex gap-2">
              <Input id="timeSpent" {...form.register('timeSpent')} placeholder="1h 30m" />
              <Button
                type="button"
                variant="secondary"
                size="icon"
                onClick={toggleTimer}
                aria-label={timerRunning ? t('timeTracking.timer.stop') : t('timeTracking.timer.start')}
              >
                {timerRunning ? <Square className="h-4 w-4" /> : <Play className="h-4 w-4" />}
              </Button>
            </div>
            {timerRunning && (
              <p className="text-xs text-text-muted">{formatDuration(timerSeconds)}</p>
            )}
            {form.formState.errors.timeSpent && (
              <p className="text-xs text-danger">{form.formState.errors.timeSpent.message}</p>
            )}
          </div>

          <div className="space-y-1">
            <Label htmlFor="remainingEstimate">{t('timeTracking.fields.remainingEstimate')}</Label>
            <Input id="remainingEstimate" {...form.register('remainingEstimate')} placeholder="2h" />
          </div>

          <div className="space-y-1">
            <Label htmlFor="startedAt">{t('timeTracking.fields.startedAt')}</Label>
            <Input id="startedAt" type="date" {...form.register('startedAt')} />
          </div>

          <div className="space-y-1">
            <Label htmlFor="comment">{t('timeTracking.fields.comment')}</Label>
            <Input id="comment" {...form.register('comment')} placeholder="What did you do?" />
          </div>

          <div className="flex justify-end gap-2 pt-2">
            <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </Button>
            <Button type="submit">{t('common.save')}</Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
