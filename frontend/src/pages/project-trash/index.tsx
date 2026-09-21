import { useParams, Link } from 'react-router'
import { Trash2, RotateCcw, ArrowLeft } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@sdlc/ui/ui'
import { ConfirmDialog, ErrorState } from '@sdlc/ui/ui'
import { useTrash, useRestoreIssue, usePurgeIssue } from '@/shared/api/hooks'

const TRASH_PAGE_SIZE = 50

export function ProjectTrashPage() {
  const { projectKey } = useParams<{ projectKey: string }>()
  const { t } = useTranslation()
  const [trashOffset, setTrashOffset] = useState(0)
  const {
    data: trashedIssues = [],
    isLoading,
    error,
    refetch,
  } = useTrash(projectKey, trashOffset, TRASH_PAGE_SIZE)
  const restoreMutation = useRestoreIssue()
  const purgeMutation = usePurgeIssue()
  const [purgeConfirmIssue, setPurgeConfirmIssue] = useState<{ id: string; key: string } | null>(
    null,
  )

  useEffect(() => {
    setTrashOffset(0)
  }, [projectKey])

  useEffect(() => {
    if (!isLoading && !error && trashedIssues.length === 0 && trashOffset > 0) {
      setTrashOffset(Math.max(0, trashOffset - TRASH_PAGE_SIZE))
    }
  }, [error, isLoading, trashedIssues.length, trashOffset])

  if (!projectKey) {
    return <div className="text-text-muted">{t('trash.noProject')}</div>
  }

  const hasPrev = trashOffset > 0
  const hasNext = trashedIssues.length === TRASH_PAGE_SIZE
  const pageFrom = trashedIssues.length > 0 ? trashOffset + 1 : trashOffset
  const pageTo = trashOffset + trashedIssues.length

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <Button variant="ghost" size="icon" asChild>
          <Link to={`/projects/${projectKey}/board`} aria-label={t('trash.backToBoard')}>
            <ArrowLeft className="h-4 w-4" />
          </Link>
        </Button>
        <Trash2 className="h-5 w-5 text-text-muted" />
        <h1 className="text-xl font-semibold">
          {t('trash.title', 'Trash')} · {projectKey}
        </h1>
      </div>

      {isLoading && (
        <div className="py-8 text-center text-sm text-text-muted">{t('trash.loading')}</div>
      )}

      {error && <ErrorState message={t('common.error')} onRetry={() => void refetch()} />}

      {!error && !isLoading && trashedIssues.length === 0 && (
        <div className="py-16 text-center text-sm text-text-muted">
          {hasPrev ? t('trash.emptyPage') : t('trash.noIssues')}
        </div>
      )}

      {trashedIssues.length > 0 && (
        <div className="overflow-hidden rounded-md border border-border bg-surface text-sm">
          <div className="hidden grid-cols-[7rem_minmax(0,1fr)_8rem_8rem_auto] gap-3 border-b border-border bg-surface-raised px-4 py-2 text-text-secondary sm:grid">
            <span>{t('trash.key')}</span>
            <span>{t('trash.summary')}</span>
            <span>{t('trash.type')}</span>
            <span>{t('trash.priority')}</span>
            <span className="text-right">{t('trash.actions')}</span>
          </div>
          {trashedIssues.map((issue) => (
            <article
              key={issue.id}
              className="grid gap-2 border-b border-border p-3 last:border-0 hover:bg-surface-raised sm:grid-cols-[7rem_minmax(0,1fr)_8rem_8rem_auto] sm:items-center sm:gap-3 sm:px-4"
            >
              <div className="font-mono text-xs text-text-secondary">{issue.key}</div>
              <div className="min-w-0 break-words font-medium sm:font-normal">{issue.summary}</div>
              <div className="text-xs text-text-secondary sm:text-sm">
                {t(`issueType.${issue.issue_type.toLowerCase()}`, {
                  defaultValue: issue.issue_type,
                })}
              </div>
              <div className="text-xs text-text-secondary sm:text-sm">
                {t(`priority.${issue.priority.toLowerCase()}`, { defaultValue: issue.priority })}
              </div>
              <div className="mt-1 grid grid-cols-2 gap-2 sm:mt-0 sm:flex sm:justify-end">
                <Button
                  variant="ghost"
                  size="sm"
                  className="min-h-11 gap-1 px-2 text-center text-xs leading-tight whitespace-normal sm:min-h-10"
                  aria-label={t('trash.restoreIssue', { key: issue.key })}
                  disabled={restoreMutation.isPending}
                  onClick={() =>
                    restoreMutation.mutate(issue.id, {
                      onSuccess: () => toast.success(t('trash.restored', { key: issue.key })),
                      onError: () => toast.error(t('trash.restoreError', { key: issue.key })),
                    })
                  }
                >
                  <RotateCcw className="h-3.5 w-3.5" />
                  {t('trash.restore', 'Restore')}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  className="min-h-11 gap-1 px-2 text-center text-xs leading-tight whitespace-normal text-danger hover:text-danger sm:min-h-10"
                  aria-label={t('trash.purgeIssue', { key: issue.key })}
                  disabled={purgeMutation.isPending}
                  onClick={() => {
                    purgeMutation.reset()
                    setPurgeConfirmIssue({ id: issue.id, key: issue.key })
                  }}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                  {t('trash.purge', 'Delete forever')}
                </Button>
              </div>
            </article>
          ))}
        </div>
      )}

      {(hasPrev || hasNext) && (
        <div className="flex items-center justify-between rounded-lg border border-border bg-surface px-3 py-2.5">
          <Button
            variant="outline"
            size="sm"
            disabled={!hasPrev || isLoading}
            onClick={() => setTrashOffset(Math.max(0, trashOffset - TRASH_PAGE_SIZE))}
          >
            {t('trash.prevPage')}
          </Button>
          <span className="text-sm text-text-muted">
            {trashedIssues.length > 0
              ? t('trash.pageInfo', { from: pageFrom, to: pageTo })
              : t('trash.emptyPage')}
          </span>
          <Button
            variant="outline"
            size="sm"
            disabled={!hasNext || isLoading}
            onClick={() => setTrashOffset(trashOffset + TRASH_PAGE_SIZE)}
          >
            {t('trash.nextPage')}
          </Button>
        </div>
      )}

      <ConfirmDialog
        open={purgeConfirmIssue !== null}
        onOpenChange={(open) => {
          if (!open) {
            setPurgeConfirmIssue(null)
            purgeMutation.reset()
          }
        }}
        isPending={purgeMutation.isPending}
        error={purgeMutation.error ? t('trash.purgeError') : null}
        title={t('trash.purge', 'Delete forever')}
        description={t('trash.purgeConfirm', { key: purgeConfirmIssue?.key })}
        onConfirm={() => {
          if (purgeConfirmIssue) {
            const { id, key } = purgeConfirmIssue
            purgeMutation.mutate(id, {
              onSuccess: () => {
                setPurgeConfirmIssue(null)
                toast.success(t('trash.purged', { key }))
              },
            })
          }
        }}
      />
    </div>
  )
}
