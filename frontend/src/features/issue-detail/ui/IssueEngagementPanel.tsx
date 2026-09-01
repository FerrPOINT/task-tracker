import { Eye, ThumbsUp } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import {
  useIssueVotes,
  useIssueWatchers,
  useVoteIssue,
  useUnvoteIssue,
  useWatchIssue,
  useUnwatchIssue,
} from '@/shared/api/hooks'
import { Button } from '@/shared/ui/button'
import { UserAvatar } from '@/shared/ui/user-avatar'

type IssueEngagementPanelProps = {
  issueId: string
  projectKey: string
  currentUserId?: string | null
  reporterId?: string | null
}

export function IssueEngagementPanel({
  issueId,
  projectKey,
  currentUserId,
  reporterId,
}: IssueEngagementPanelProps) {
  const { t } = useTranslation()
  const votesQuery = useIssueVotes(issueId)
  const watchersQuery = useIssueWatchers(issueId)
  const vote = useVoteIssue(issueId, projectKey)
  const unvote = useUnvoteIssue(issueId, projectKey)
  const watch = useWatchIssue(issueId, projectKey)
  const unwatch = useUnwatchIssue(issueId, projectKey)

  const votes = votesQuery.data?.votes ?? []
  const voteCount = votesQuery.data?.count ?? votes.length
  const watchers = watchersQuery.data ?? []
  const hasVoted = !!currentUserId && votes.some((v) => v.user_id === currentUserId)
  const isWatching = !!currentUserId && watchers.some((w) => w.user_id === currentUserId)
  const isOwnIssue = !!currentUserId && currentUserId === reporterId
  const cannotAddVote = isOwnIssue && !hasVoted
  const visibleWatchers = watchers.slice(0, 5)
  const hiddenWatchers = Math.max(0, watchers.length - visibleWatchers.length)

  return (
    <div className="space-y-4" data-testid="issue-engagement-panel">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <ThumbsUp className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden />
          <div className="min-w-0">
            <p className="text-sm font-medium text-text-primary">{t('engagement.votes')}</p>
            <p className="text-xs text-muted-foreground">
              {votesQuery.isLoading
                ? t('common.loading')
                : t('engagement.count', { count: voteCount })}
            </p>
          </div>
        </div>
        <Button
          type="button"
          variant={hasVoted ? 'secondary' : 'outline'}
          size="sm"
          disabled={!currentUserId || cannotAddVote || vote.isPending || unvote.isPending}
          onClick={() => (hasVoted ? unvote.mutate() : vote.mutate())}
          aria-label={hasVoted ? t('engagement.unvote') : t('engagement.vote')}
          title={cannotAddVote ? t('engagement.ownIssue') : undefined}
        >
          {hasVoted ? t('engagement.voted') : t('engagement.vote')}
        </Button>
      </div>

      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <Eye className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden />
          <div className="min-w-0">
            <p className="text-sm font-medium text-text-primary">{t('engagement.watchers')}</p>
            {watchersQuery.isLoading ? (
              <p className="text-xs text-muted-foreground">{t('common.loading')}</p>
            ) : watchers.length > 0 ? (
              <div className="mt-1 flex items-center">
                {visibleWatchers.map((w) => (
                  <span
                    key={w.user_id}
                    className="-mr-2 rounded-full ring-2 ring-surface last:mr-0"
                    title={w.display_name || w.username}
                  >
                    <UserAvatar name={w.display_name || w.username} userId={w.user_id} />
                  </span>
                ))}
                {hiddenWatchers > 0 && (
                  <span className="ml-1 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                    +{hiddenWatchers}
                  </span>
                )}
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">{t('engagement.noWatchers')}</p>
            )}
          </div>
        </div>
        <Button
          type="button"
          variant={isWatching ? 'secondary' : 'outline'}
          size="sm"
          disabled={!currentUserId || watch.isPending || unwatch.isPending}
          onClick={() => (isWatching ? unwatch.mutate() : watch.mutate())}
          aria-label={isWatching ? t('engagement.unwatch') : t('engagement.watch')}
        >
          {isWatching ? t('engagement.watching') : t('engagement.watch')}
        </Button>
      </div>
    </div>
  )
}
