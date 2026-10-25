import { useRef, useState, type RefObject } from 'react'
import { useTranslation } from 'react-i18next'
import { format } from 'date-fns'
import { Pencil, Trash2 } from 'lucide-react'
import {
  EmptyState,
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
  Button,
  ConfirmDialog,
  Card,
  CardContent,
  DescriptionItem,
  ResourceListTitle,
} from '@pdlc/ui/ui'

import { formatDuration } from '@/shared/lib/time'
import type { Worklog } from '@/entities/worklog/model'

interface WorklogTabProps {
  worklogs: Worklog[]
  onEdit: (worklog: Worklog) => void
  onDelete: (id: string) => Promise<unknown>
  currentUserId: string
  fallbackFocusRef?: RefObject<HTMLElement | null>
}

export function WorklogTab({
  worklogs,
  onEdit,
  onDelete,
  currentUserId,
  fallbackFocusRef,
}: WorklogTabProps) {
  const { t } = useTranslation()
  const [deletingId, setDeletingId] = useState<string | null>(null)
  const [deletePending, setDeletePending] = useState(false)
  const [deleteError, setDeleteError] = useState<string | null>(null)
  const returnFocusRef = useRef<HTMLElement | null>(null)

  const openDelete = (id: string, trigger: HTMLButtonElement) => {
    returnFocusRef.current = trigger
    setDeleteError(null)
    setDeletingId(id)
  }

  const confirmDelete = async () => {
    if (!deletingId) return
    setDeletePending(true)
    setDeleteError(null)
    try {
      await onDelete(deletingId)
      returnFocusRef.current = fallbackFocusRef?.current ?? null
      setDeletingId(null)
    } catch (error) {
      setDeleteError(error instanceof Error ? error.message : t('common.error'))
    } finally {
      setDeletePending(false)
    }
  }

  const total = worklogs.reduce((sum, w) => sum + w.timeSpentSeconds, 0)
  const deleteDialog = (
    <ConfirmDialog
      open={deletingId !== null}
      onOpenChange={(open) => {
        if (!open) {
          setDeletingId(null)
          setDeleteError(null)
        }
      }}
      title={t('timeTracking.deleteWorklog')}
      description={t('timeTracking.deleteConfirm')}
      isPending={deletePending}
      error={deleteError}
      returnFocusRef={returnFocusRef}
      fallbackFocusRef={fallbackFocusRef}
      onConfirm={() => void confirmDelete()}
    />
  )

  return (
    <>
      {deleteDialog}
      {worklogs.length === 0 ? (
        <EmptyState message={t('timeTracking.worklog.empty')} />
      ) : (
        <div className="space-y-4">
          <Card data-testid="worklog-table" className="hidden 2xl:block">
            <Table className="min-w-[720px]">
              <TableHeader>
                <TableRow>
                  <TableHead className="min-w-[110px] whitespace-nowrap">
                    {t('timeTracking.worklog.user')}
                  </TableHead>
                  <TableHead className="min-w-[110px] whitespace-nowrap">
                    {t('timeTracking.worklog.started')}
                  </TableHead>
                  <TableHead className="min-w-[100px] whitespace-nowrap">
                    {t('timeTracking.worklog.timeSpent')}
                  </TableHead>
                  <TableHead className="min-w-[100px] whitespace-nowrap">
                    {t('timeTracking.worklog.remaining')}
                  </TableHead>
                  <TableHead className="min-w-[180px] whitespace-nowrap">
                    {t('timeTracking.worklog.comment')}
                  </TableHead>
                  <TableHead className="min-w-[90px] whitespace-nowrap text-right">
                    {t('timeTracking.worklog.actions')}
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {worklogs.map((w) => (
                  <TableRow key={w.id}>
                    <TableCell className="whitespace-nowrap font-medium">
                      {w.userDisplayName}
                    </TableCell>
                    <TableCell className="whitespace-nowrap">
                      {format(new Date(w.startedAt), 'yyyy-MM-dd')}
                    </TableCell>
                    <TableCell className="whitespace-nowrap">
                      {formatDuration(w.timeSpentSeconds)}
                    </TableCell>
                    <TableCell className="whitespace-nowrap">-</TableCell>
                    <TableCell className="max-w-[200px] truncate">{w.comment ?? '-'}</TableCell>
                    <TableCell className="whitespace-nowrap text-right">
                      {w.userId === currentUserId && (
                        <div className="flex items-center justify-end gap-1">
                          <Button
                            variant="ghost"
                            size="icon"

                            onClick={() => onEdit(w)}
                            aria-label={t('timeTracking.editWorklog')}
                          >
                            <Pencil className="h-4 w-4" />
                          </Button>
                          <Button
                            variant="destructive-outline"
                            size="icon"

                            onClick={(event) => openDelete(w.id, event.currentTarget)}
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
          </Card>

          <div className="space-y-3 2xl:hidden">
            {worklogs.map((w) => (
              <Card key={w.id}>
                <CardContent className="space-y-2">
                  <div className="flex items-start justify-between gap-3">
                    <ResourceListTitle asChild>
                      <div className="min-w-0 break-words">{w.userDisplayName}</div>
                    </ResourceListTitle>
                    {w.userId === currentUserId && (
                      <div className="flex items-center gap-1">
                        <Button
                          variant="ghost"
                          size="icon"

                          onClick={() => onEdit(w)}
                          aria-label={t('timeTracking.editWorklog')}
                        >
                          <Pencil className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="destructive-outline"
                          size="icon"

                          onClick={(event) => openDelete(w.id, event.currentTarget)}
                          aria-label={t('timeTracking.deleteWorklog')}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>
                    )}
                  </div>
                  <dl className="grid gap-1">
                    <DescriptionItem label={t('timeTracking.worklog.started')} layout="inline">
                      {format(new Date(w.startedAt), 'yyyy-MM-dd')}
                    </DescriptionItem>
                    <DescriptionItem label={t('timeTracking.worklog.timeSpent')} layout="inline">
                      {formatDuration(w.timeSpentSeconds)}
                    </DescriptionItem>
                    <DescriptionItem label={t('timeTracking.worklog.remaining')} layout="inline">
                      -
                    </DescriptionItem>
                    {w.comment && (
                      <DescriptionItem label={t('timeTracking.worklog.comment')} layout="inline">
                        {w.comment}
                      </DescriptionItem>
                    )}
                  </dl>
                </CardContent>
              </Card>
            ))}
          </div>

          <div className="text-sm text-text-secondary">
            {t('timeTracking.worklog.totalLogged')}:{' '}
            <span className="font-semibold text-text-primary">{formatDuration(total)}</span>
          </div>
        </div>
      )}
    </>
  )
}
