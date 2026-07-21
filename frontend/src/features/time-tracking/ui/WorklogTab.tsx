import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { format } from 'date-fns'
import { Pencil, Trash2 } from 'lucide-react'
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@/shared/ui/table'
import { Button } from '@/shared/ui/button'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/shared/ui/alert-dialog'
import { Card, CardContent } from '@/shared/ui/card'
import { formatDuration } from '@/shared/lib/time'
import type { Worklog } from '@/entities/worklog/model'

interface WorklogTabProps {
  worklogs: Worklog[]
  onEdit: (worklog: Worklog) => void
  onDelete: (id: string) => void
  currentUserId: string
}

export function WorklogTab({ worklogs, onEdit, onDelete, currentUserId }: WorklogTabProps) {
  const { t } = useTranslation()
  const [deletingId, setDeletingId] = useState<string | null>(null)

  const total = worklogs.reduce((sum, w) => sum + w.timeSpentSeconds, 0)

  if (worklogs.length === 0) {
    return <p className="text-sm text-text-muted">{t('timeTracking.worklog.empty')}</p>
  }

  return (
    <div className="space-y-4">
      <div className="hidden overflow-x-auto rounded-md border border-border md:block" data-testid="worklog-table">
        <Table className="min-w-[720px]">
          <TableHeader>
            <TableRow>
              <TableHead className="min-w-[110px] whitespace-nowrap">{t('timeTracking.worklog.user')}</TableHead>
              <TableHead className="min-w-[110px] whitespace-nowrap">{t('timeTracking.worklog.started')}</TableHead>
              <TableHead className="min-w-[100px] whitespace-nowrap">{t('timeTracking.worklog.timeSpent')}</TableHead>
              <TableHead className="min-w-[100px] whitespace-nowrap">{t('timeTracking.worklog.remaining')}</TableHead>
              <TableHead className="min-w-[180px] whitespace-nowrap">{t('timeTracking.worklog.comment')}</TableHead>
              <TableHead className="min-w-[90px] whitespace-nowrap text-right">{t('timeTracking.worklog.actions')}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {worklogs.map((w) => (
              <TableRow key={w.id}>
                <TableCell className="whitespace-nowrap font-medium">{w.userDisplayName}</TableCell>
                <TableCell className="whitespace-nowrap">{format(new Date(w.startedAt), 'yyyy-MM-dd')}</TableCell>
                <TableCell className="whitespace-nowrap">{formatDuration(w.timeSpentSeconds)}</TableCell>
                <TableCell className="whitespace-nowrap">
                  {w.remainingEstimateSeconds !== null
                    ? formatDuration(w.remainingEstimateSeconds)
                    : '-'}
                </TableCell>
                <TableCell className="max-w-[200px] truncate">{w.comment ?? '-'}</TableCell>
                <TableCell className="whitespace-nowrap text-right">
                  {w.userId === currentUserId && (
                    <div className="flex items-center justify-end gap-1">
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8"
                        onClick={() => onEdit(w)}
                        aria-label={t('timeTracking.editWorklog')}
                      >
                        <Pencil className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 text-danger hover:opacity-80"
                        onClick={() => setDeletingId(w.id)}
                        aria-label={t('timeTracking.deleteWorklog')}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>

      {worklogs.map((w) => (
        <AlertDialog
          key={`desktop-${w.id}`}
          open={deletingId === w.id}
          onOpenChange={(open) => setDeletingId(open ? w.id : null)}
        >
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>{t('timeTracking.deleteWorklog')}</AlertDialogTitle>
              <AlertDialogDescription>{t('timeTracking.deleteConfirm')}</AlertDialogDescription>
            </AlertDialogHeader>
            <div className="flex justify-end gap-2">
              <AlertDialogCancel onClick={() => setDeletingId(null)}>
                {t('common.cancel')}
              </AlertDialogCancel>
              <AlertDialogAction onClick={() => onDelete(w.id)}>
                {t('common.delete')}
              </AlertDialogAction>
            </div>
          </AlertDialogContent>
        </AlertDialog>
      ))}

      <div className="space-y-3 md:hidden">
        {worklogs.map((w) => (
          <Card key={w.id}>
            <CardContent className="space-y-2 p-3">
              <div className="flex items-start justify-between gap-3">
                <div className="font-medium text-text-primary">{w.userDisplayName}</div>
                {w.userId === currentUserId && (
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8"
                      onClick={() => onEdit(w)}
                      aria-label={t('timeTracking.editWorklog')}
                    >
                      <Pencil className="h-4 w-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 text-danger hover:opacity-80"
                      onClick={() => setDeletingId(w.id)}
                      aria-label={t('timeTracking.deleteWorklog')}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                )}
              </div>
              <div className="grid grid-cols-2 gap-x-3 gap-y-1 text-sm">
                <span className="text-text-muted">{t('timeTracking.worklog.started')}</span>
                <span className="text-text-primary">{format(new Date(w.startedAt), 'yyyy-MM-dd')}</span>
                <span className="text-text-muted">{t('timeTracking.worklog.timeSpent')}</span>
                <span className="text-text-primary">{formatDuration(w.timeSpentSeconds)}</span>
                <span className="text-text-muted">{t('timeTracking.worklog.remaining')}</span>
                <span className="text-text-primary">
                  {w.remainingEstimateSeconds !== null
                    ? formatDuration(w.remainingEstimateSeconds)
                    : '-'}
                </span>
                {w.comment && (
                  <>
                    <span className="text-text-muted">{t('timeTracking.worklog.comment')}</span>
                    <span className="text-text-primary">{w.comment}</span>
                  </>
                )}
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <div className="text-sm text-text-secondary">
        {t('timeTracking.worklog.totalLogged')}: {' '}
        <span className="font-semibold text-text-primary">{formatDuration(total)}</span>
      </div>
    </div>
  )
}
