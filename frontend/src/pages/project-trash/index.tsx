import { useParams, Link } from 'react-router'
import { Trash2, RotateCcw, ArrowLeft } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useEffect, useState } from 'react'
import { Button } from '@/shared/ui/button'
import { ConfirmDialog, ErrorState } from '@/shared/ui/async-states'
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
  const [purgeConfirmId, setPurgeConfirmId] = useState<string | null>(null)

  useEffect(() => {
    setTrashOffset(0)
  }, [projectKey])

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
        <Button variant="ghost" size="icon" asChild aria-label={t('trash.title')}>
          <Link to={`/projects/${projectKey}/board`}>
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
        <div className="overflow-hidden rounded-lg border border-border">
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead className="border-b border-border bg-surface-raised text-text-secondary">
                <tr>
                  <th className="px-4 py-2 text-left font-medium">{t('trash.key')}</th>
                  <th className="px-4 py-2 text-left font-medium">{t('trash.summary')}</th>
                  <th className="px-4 py-2 text-left font-medium">{t('trash.type')}</th>
                  <th className="px-4 py-2 text-left font-medium">{t('trash.priority')}</th>
                  <th className="px-4 py-2 text-right font-medium">{t('trash.actions')}</th>
                </tr>
              </thead>
              <tbody>
                {trashedIssues.map((issue) => (
                  <tr
                    key={issue.id}
                    className="border-b border-border last:border-0 hover:bg-surface-raised"
                  >
                    <td className="px-4 py-3 font-mono text-xs text-text-secondary">{issue.key}</td>
                    <td className="px-4 py-3">{issue.summary}</td>
                    <td className="px-4 py-3 text-text-secondary">{issue.issue_type}</td>
                    <td className="px-4 py-3 text-text-secondary">{issue.priority}</td>
                    <td className="px-4 py-3">
                      <div className="flex justify-end gap-2">
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-7 gap-1 px-2 text-xs"
                          disabled={restoreMutation.isPending}
                          onClick={() => restoreMutation.mutate(issue.id)}
                        >
                          <RotateCcw className="h-3 w-3" />
                          {t('trash.restore', 'Restore')}
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-7 gap-1 px-2 text-xs text-danger hover:text-danger"
                          disabled={purgeMutation.isPending}
                          onClick={() => setPurgeConfirmId(issue.id)}
                        >
                          <Trash2 className="h-3 w-3" />
                          {t('trash.purge', 'Delete forever')}
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
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
        open={purgeConfirmId !== null}
        onOpenChange={(open) => !open && setPurgeConfirmId(null)}
        title={t('trash.purge', 'Delete forever')}
        description={t(
          'trash.purgeConfirm',
          'Permanently delete this issue? This action cannot be undone.',
        )}
        onConfirm={() => {
          if (purgeConfirmId) {
            purgeMutation.mutate(purgeConfirmId)
          }
          setPurgeConfirmId(null)
        }}
      />
    </div>
  )
}
