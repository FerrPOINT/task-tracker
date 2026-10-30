import {
  ResourceStateView,
  LoadingState,
  EmptyState,
  PageNavigation,
  ResourceListRow,
  ResourceListHeader,
  ResourceList,
  PageHeader,
  ProjectAvatar,
  toast,
  Button,
  ConfirmDialog,
  ErrorState,
} from '@pdlc/ui/ui'
import { ProjectLink as Link } from '@/shared/lib/project-navigation'
import { useParams, useSearchParams } from 'react-router'
import { Trash2, RotateCcw, ArrowLeft } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useCallback, useEffect, useState } from 'react'

import { useTrash, useRestoreIssue, usePurgeIssue } from '@/shared/api/hooks'

const TRASH_PAGE_SIZE = 50

export function ProjectTrashPage() {
  const { projectKey } = useParams<{ projectKey: string }>()
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const pageParam = searchParams.get('page')
  const parsedPage = Number(pageParam)
  const hasValidPage =
    pageParam !== null &&
    Number.isSafeInteger(parsedPage) &&
    parsedPage > 0 &&
    Number.isSafeInteger((parsedPage - 1) * TRASH_PAGE_SIZE)
  const page = hasValidPage ? parsedPage : 1
  const trashOffset = (page - 1) * TRASH_PAGE_SIZE
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

  const updatePage = useCallback(
    (nextPage: number, replace = false) => {
      setSearchParams(
        (current) => {
          const next = new URLSearchParams(current)
          if (nextPage > 1) next.set('page', String(nextPage))
          else next.delete('page')
          return next
        },
        { replace },
      )
    },
    [setSearchParams],
  )

  useEffect(() => {
    const canonicalPage = hasValidPage && parsedPage > 1 ? String(parsedPage) : null
    if (pageParam === canonicalPage) return

    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current)
        if (canonicalPage) next.set('page', canonicalPage)
        else next.delete('page')
        return next
      },
      { replace: true },
    )
  }, [hasValidPage, pageParam, parsedPage, setSearchParams])

  useEffect(() => {
    if (!isLoading && !error && trashedIssues.length === 0 && trashOffset > 0) {
      updatePage(1, true)
    }
  }, [error, isLoading, trashedIssues.length, trashOffset, updatePage])

  if (!projectKey) {
    return (
      <ResourceStateView
        scope="local"
        state={{ kind: 'not-found', title: t('trash.noProject'), headingLevel: 1 }}
      />
    )
  }

  const hasPrev = trashOffset > 0
  const hasNext = trashedIssues.length === TRASH_PAGE_SIZE
  const pageFrom = trashedIssues.length > 0 ? trashOffset + 1 : trashOffset
  const pageTo = trashOffset + trashedIssues.length

  return (
    <div className="space-y-4">
      <PageHeader
        title={`${t('trash.title', 'Trash')} · ${projectKey ?? ''}`}
        leading={
          <>
            <Button variant="ghost" size="icon" asChild>
              <Link to={`/projects/${projectKey}/board`} aria-label={t('trash.backToBoard')}>
                <ArrowLeft className="h-4 w-4" />
              </Link>
            </Button>
            <Trash2 className="h-5 w-5 text-text-muted" />
            <ProjectAvatar projectKey={projectKey ?? ''} />
          </>
        }
      />

      {isLoading && <LoadingState scope="local" message={t('trash.loading')} />}

      {error && <ErrorState message={t('common.error')} onRetry={() => void refetch()} />}

      {!error && !isLoading && trashedIssues.length === 0 && (
        <EmptyState message={hasPrev ? t('trash.emptyPage') : t('trash.noIssues')} />
      )}

      {trashedIssues.length > 0 && (
        <ResourceList
          variant="bordered"
          className="text-sm lg:grid lg:grid-cols-[7rem_minmax(0,1fr)_8rem_8rem_auto]"
        >
          <ResourceListHeader className="hidden gap-3 lg:col-span-full lg:grid lg:grid-cols-subgrid">
            <span>{t('trash.key')}</span>
            <span>{t('trash.summary')}</span>
            <span>{t('trash.type')}</span>
            <span>{t('trash.priority')}</span>
            <span className="text-right">{t('trash.actions')}</span>
          </ResourceListHeader>
          {trashedIssues.map((issue) => (
            <ResourceListRow asChild key={issue.id}>
              <article className="grid gap-2 lg:col-span-full lg:grid-cols-subgrid lg:items-center lg:gap-3">
                <div className="font-mono text-xs text-text-secondary">{issue.key}</div>
                <div className="min-w-0 break-words font-medium lg:font-normal">
                  {issue.summary}
                </div>
                <div className="text-xs text-text-secondary lg:text-sm">
                  {t(`issueType.${issue.issue_type.toLowerCase()}`, {
                    defaultValue: issue.issue_type,
                  })}
                </div>
                <div className="text-xs text-text-secondary lg:text-sm">
                  {t(`priority.${issue.priority.toLowerCase()}`, { defaultValue: issue.priority })}
                </div>
                <div className="mt-1 grid grid-cols-2 gap-2 lg:mt-0 lg:flex lg:justify-end">
                  <Button
                    variant="ghost"
                    size="sm"

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
                    variant="destructive-outline"
                    size="sm"

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
            </ResourceListRow>
          ))}
        </ResourceList>
      )}

      {(hasPrev || hasNext) && (
        <PageNavigation
          page={page}
          hasNext={hasNext}
          pending={isLoading}
          onPageChange={updatePage}
          label={t('trash.title')}
          previousLabel={t('trash.prevPage')}
          nextLabel={t('trash.nextPage')}
          summary={
            trashedIssues.length > 0
              ? t('trash.pageInfo', { from: pageFrom, to: pageTo })
              : t('trash.emptyPage')
          }
        />
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
