import {
  TextLink,
  SectionHeader,
  Select,
  ResourceList,
  ResourceListRow,
  Button,
  Input,
  toast,
  Notice,
  ResourceStateView,
  resourceErrorState,
  EmptyState,
  Fieldset,
} from '@pdlc/ui/ui'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link2, Plus, X } from 'lucide-react'
import { ProjectLink as Link } from '@/shared/lib/project-navigation'

import { useIssueLinks, useCreateIssueLink, useDeleteIssueLink } from '@/shared/api/hooks'

const LINK_TYPES = ['blocks', 'duplicates', 'relates'] as const

export function LinkEditor({ issueId, currentKey }: { issueId: string; currentKey: string }) {
  return <IssueLinkEditor key={issueId} issueId={issueId} currentKey={currentKey} />
}

function IssueLinkEditor({ issueId, currentKey }: { issueId: string; currentKey: string }) {
  const { t } = useTranslation()
  const query = useIssueLinks(issueId)
  const links = query.data ?? []
  const state = query.error
    ? resourceErrorState(query.error, () => void query.refetch(), {
        hasData: query.data !== undefined,
      })
    : query.data === undefined
      ? { kind: 'loading' as const, message: t('common.loading') }
      : { kind: 'ready' as const }
  const canEdit = state.kind === 'ready'
  const create = useCreateIssueLink(issueId)
  const remove = useDeleteIssueLink()
  const [adding, setAdding] = useState(false)
  const [targetKey, setTargetKey] = useState('')
  const [linkType, setLinkType] = useState<(typeof LINK_TYPES)[number]>('relates')
  const [error, setError] = useState('')

  const onAdd = async () => {
    if (!canEdit) return
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
      <SectionHeader
        level={3}
        title={
          <>
            <Link2 className="h-4 w-4" aria-hidden />
            {t('links.title')}
          </>
        }
      />

      <ResourceStateView state={state} scope="local">
        <Fieldset legend={t('links.title')} legendVisuallyHidden disabled={!canEdit}>
          {links.length > 0 && (
            <ResourceList asChild>
              <ul>
                {links.map((l) => {
                  const isSource = l.source_id === issueId
                  const otherKey = isSource ? l.target_key : l.source_key
                  const otherId = isSource ? l.target_id : l.source_id
                  const label = isSource
                    ? t(`links.type.${l.link_type}`)
                    : t(`links.typeInverse.${l.link_type}`)
                  return (
                    <ResourceListRow asChild key={l.id}>
                      <li className="flex items-center justify-between gap-2 text-sm">
                        <span className="min-w-0 break-words">
                          <span className="text-muted-foreground">{label}</span>{' '}
                          <TextLink asChild>
                            <Link to={`/issues/${otherId}`} className="font-medium">
                              {otherKey}
                            </Link>
                          </TextLink>
                        </span>
                        <Button
                          size="icon"
                          variant="destructive-outline"
                          type="button"
                          aria-label={t('links.delete', { key: otherKey })}
                          onClick={() =>
                            canEdit &&
                            remove.mutate(l.id, {
                              onSuccess: () => toast.success(t('common.saved')),
                              onError: (error) => toast.error(error.message),
                            })
                          }
                          className="shrink-0"
                        >
                          <X className="h-3.5 w-3.5" aria-hidden />
                        </Button>
                      </li>
                    </ResourceListRow>
                  )
                })}
              </ul>
            </ResourceList>
          )}
          {links.length === 0 && <EmptyState message={t('links.none')} />}

          {adding ? (
            <div className="flex flex-wrap gap-1">
              <Input
                value={targetKey}
                onChange={(e) => setTargetKey(e.target.value)}
                aria-label={t('links.targetKey')}
                placeholder={currentKey ? `${currentKey.split('-')[0]}-42` : 'TT-42'}
                className="min-w-0 flex-1 text-xs sm:w-32 sm:flex-none"
                data-testid="link-target-input"
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault()
                    void onAdd()
                  }
                }}
              />
              <Select
                value={linkType}
                disabled={!canEdit}
                onValueChange={(nextValue) => setLinkType(nextValue as (typeof LINK_TYPES)[number])}
                className="w-auto"
                aria-label={t('links.typeLabel')}
                options={[
                  ...(LINK_TYPES.map((lt) => ({
                    value: String(lt),
                    label: String(t(`links.type.${lt}`)),
                  })) ?? []),
                ]}
              />
              <Button
                type="button"
                size="sm"

                onClick={() => void onAdd()}
                disabled={create.isPending || !targetKey.trim()}
                data-testid="link-submit"
              >
                {create.isPending ? t('common.saving') : t('links.submit')}
              </Button>
              {error && (
                <Notice role="alert" className="w-full" tone="danger" appearance="inline">
                  {error}
                </Notice>
              )}
            </div>
          ) : (
            <Button type="button" variant="ghost" size="sm" onClick={() => setAdding(true)}>
              <Plus className="mr-1 h-3 w-3" aria-hidden />
              {t('links.add')}
            </Button>
          )}
        </Fieldset>
      </ResourceStateView>
    </div>
  )
}
