import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link2, Plus, X } from 'lucide-react'
import { Link } from 'react-router'
import { useIssueLinks, useCreateIssueLink, useDeleteIssueLink } from '@/shared/api/hooks'
import { Button } from '@sdlc/ui/ui'
import { Input } from '@sdlc/ui/ui'
import { toast } from 'sonner'

const LINK_TYPES = ['blocks', 'duplicates', 'relates'] as const

export function LinkEditor({ issueId, currentKey }: { issueId: string; currentKey: string }) {
  const { t } = useTranslation()
  const { data: links = [] } = useIssueLinks(issueId)
  const create = useCreateIssueLink(issueId)
  const remove = useDeleteIssueLink()
  const [adding, setAdding] = useState(false)
  const [targetKey, setTargetKey] = useState('')
  const [linkType, setLinkType] = useState<(typeof LINK_TYPES)[number]>('relates')
  const [error, setError] = useState('')

  const onAdd = async () => {
    setError('')
    try {
      await create.mutateAsync({ targetKey: targetKey.trim(), linkType })
      setTargetKey('')
      setAdding(false)
      toast.success(t('common.saved'))
    } catch {
      setError(t('links.notFound', { key: targetKey.trim() }))
    }
  }

  return (
    <div className="space-y-2" data-testid="link-editor">
      <h3 className="flex items-center gap-2 text-sm font-semibold">
        <Link2 className="h-4 w-4" aria-hidden />
        {t('links.title')}
      </h3>

      {links.length > 0 && (
        <ul className="space-y-1">
          {links.map((l) => {
            const isSource = l.source_id === issueId
            const otherKey = isSource ? l.target_key : l.source_key
            const otherId = isSource ? l.target_id : l.source_id
            const label = isSource
              ? t(`links.type.${l.link_type}`)
              : t(`links.typeInverse.${l.link_type}`)
            return (
              <li key={l.id} className="flex min-h-10 items-center justify-between gap-2 text-sm">
                <span className="min-w-0 break-words">
                  <span className="text-muted-foreground">{label}</span>{' '}
                  <Link
                    to={`/issues/${otherId}`}
                    className="font-medium text-accent hover:underline"
                  >
                    {otherKey}
                  </Link>
                </span>
                <button
                  type="button"
                  aria-label={t('links.delete', { key: otherKey })}
                  onClick={() =>
                    remove.mutate(l.id, {
                      onSuccess: () => toast.success(t('common.saved')),
                      onError: (error) => toast.error(error.message),
                    })
                  }
                  className="inline-flex min-h-10 min-w-10 shrink-0 items-center justify-center rounded text-muted-foreground hover:text-destructive focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                >
                  <X className="h-3.5 w-3.5" aria-hidden />
                </button>
              </li>
            )
          })}
        </ul>
      )}
      {links.length === 0 && <p className="text-xs text-muted-foreground">{t('links.none')}</p>}

      {adding ? (
        <div className="flex flex-wrap gap-1">
          <Input
            value={targetKey}
            onChange={(e) => setTargetKey(e.target.value)}
            aria-label={t('links.targetKey')}
            placeholder={currentKey ? `${currentKey.split('-')[0]}-42` : 'TT-42'}
            className="min-h-10 min-w-0 flex-1 text-xs sm:w-32 sm:flex-none"
            data-testid="link-target-input"
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                void onAdd()
              }
            }}
          />
          <select
            value={linkType}
            onChange={(e) => setLinkType(e.target.value as (typeof LINK_TYPES)[number])}
            className="min-h-10 rounded-md border border-border bg-background px-2 text-xs"
            aria-label={t('links.typeLabel')}
          >
            {LINK_TYPES.map((lt) => (
              <option key={lt} value={lt}>
                {t(`links.type.${lt}`)}
              </option>
            ))}
          </select>
          <Button
            type="button"
            size="sm"
            className="h-10 lg:h-8"
            onClick={() => void onAdd()}
            disabled={create.isPending || !targetKey.trim()}
            data-testid="link-submit"
          >
            {create.isPending ? t('common.saving') : t('links.submit')}
          </Button>
          {error && (
            <p role="alert" className="w-full text-xs text-destructive">
              {error}
            </p>
          )}
        </div>
      ) : (
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-10 px-2 text-xs lg:h-8"
          onClick={() => setAdding(true)}
        >
          <Plus className="mr-1 h-3 w-3" aria-hidden />
          {t('links.add')}
        </Button>
      )}
    </div>
  )
}
