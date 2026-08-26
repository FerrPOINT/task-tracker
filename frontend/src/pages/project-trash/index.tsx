import { useParams, Link } from 'react-router'
import { Trash2, RotateCcw, ArrowLeft } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { useTrash, useRestoreIssue, usePurgeIssue } from '@/shared/api/hooks'

export function ProjectTrashPage() {
  const { projectKey } = useParams<{ projectKey: string }>()
  const { t } = useTranslation()
  const { data: trashedIssues = [], isLoading } = useTrash(projectKey)
  const restoreMutation = useRestoreIssue()
  const purgeMutation = usePurgeIssue()

  if (!projectKey) {
    return <div className="text-text-muted">{t('trash.noProject')}</div>
  }

  return (
    <div className="mx-auto max-w-4xl space-y-4">
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

      {!isLoading && trashedIssues.length === 0 && (
        <div className="py-16 text-center text-sm text-text-muted">
          {t('trash.noIssues')}
        </div>
      )}

      {trashedIssues.length > 0 && (
        <div className="overflow-hidden rounded-lg border border-border">
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
                  <td className="px-4 py-3 font-mono text-xs text-text-secondary">
                    {issue.key}
                  </td>
                  <td className="px-4 py-3">{issue.summary}</td>
                  <td className="px-4 py-3 text-text-secondary">
                    {issue.issue_type}
                  </td>
                  <td className="px-4 py-3 text-text-secondary">
                    {issue.priority}
                  </td>
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
                        onClick={() => {
                          if (
                            window.confirm(
                              t(
                                'trash.purgeConfirm',
                                'Permanently delete this issue? This action cannot be undone.',
                              ),
                            )
                          ) {
                            purgeMutation.mutate(issue.id)
                          }
                        }}
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
      )}
    </div>
  )
}