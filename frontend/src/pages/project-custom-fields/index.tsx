import { useState } from 'react'
import { useParams } from 'react-router'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Trash2 } from 'lucide-react'
import { Button, ConfirmDialog, ErrorState } from '@sdlc/ui/ui'
import { Card, CardContent, CardHeader, CardTitle } from '@sdlc/ui/ui'
import {
  useCreateCustomField,
  useDeleteCustomField,
  useProjectCustomFields,
} from '@/shared/api/hooks'
import type { CustomFieldInput, CustomFieldType } from '@/api/custom-fields'

const types: CustomFieldType[] = ['text', 'number', 'select', 'multi-select', 'date']
const initial: CustomFieldInput = { name: '', field_type: 'text', options: [], is_required: false }
export function ProjectCustomFieldsPage() {
  const { t } = useTranslation()
  const { projectKey = '' } = useParams()
  const [draft, setDraft] = useState<CustomFieldInput>(initial)
  const [optionsText, setOptionsText] = useState('')
  const [pendingDelete, setPendingDelete] = useState<{ id: string; name: string } | null>(null)
  const fields = useProjectCustomFields(projectKey)
  const create = useCreateCustomField(projectKey)
  const remove = useDeleteCustomField(projectKey)
  const needsOptions = draft.field_type === 'select' || draft.field_type === 'multi-select'
  const options = optionsText
    .split(',')
    .map((value) => value.trim())
    .filter(Boolean)
  return (
    <div className="space-y-5">
      <div>
        <h1 className="text-2xl font-semibold">{t('customFields.title')}</h1>
        <p className="text-sm text-text-muted">{t('customFields.description', { projectKey })}</p>
      </div>
      <div className="grid gap-6 lg:grid-cols-[minmax(18rem,28rem)_minmax(0,1fr)] lg:items-start">
        <Card>
          <CardHeader>
            <CardTitle className="text-base">{t('customFields.addTitle')}</CardTitle>
          </CardHeader>
          <CardContent>
            <form
              className="grid gap-3"
              onSubmit={(e) => {
                e.preventDefault()
                if (!draft.name.trim()) return
                if (needsOptions && options.length === 0) return
                create.mutate(
                  { ...draft, name: draft.name.trim(), options: needsOptions ? options : [] },
                  {
                    onSuccess: () => {
                      setDraft(initial)
                      setOptionsText('')
                      toast.success(t('customFields.created'))
                    },
                  },
                )
              }}
            >
              {create.error && (
                <p role="alert" className="text-sm text-danger">
                  {t('customFields.createError')}
                </p>
              )}
              <input
                aria-label={t('customFields.fieldName')}
                className="min-h-11 rounded border border-border bg-background p-2 sm:min-h-10"
                placeholder={t('customFields.fieldName')}
                required
                value={draft.name}
                onChange={(e) => {
                  create.reset()
                  setDraft({ ...draft, name: e.target.value })
                }}
              />
              <select
                aria-label={t('customFields.fieldType')}
                className="min-h-11 rounded border border-border bg-background p-2 sm:min-h-10"
                value={draft.field_type}
                onChange={(e) => {
                  create.reset()
                  setDraft({ ...draft, field_type: e.target.value as CustomFieldType })
                }}
              >
                {types.map((type) => (
                  <option key={type} value={type}>
                    {t(`customFields.types.${type}`)}
                  </option>
                ))}
              </select>
              {needsOptions && (
                <input
                  aria-label={t('customFields.options')}
                  className="min-h-11 rounded border border-border bg-background p-2 sm:min-h-10"
                  placeholder={t('customFields.options')}
                  value={optionsText}
                  onChange={(e) => {
                    create.reset()
                    setOptionsText(e.target.value)
                  }}
                />
              )}
              <label className="flex min-h-11 items-center gap-2 text-sm sm:min-h-10">
                <input
                  type="checkbox"
                  className="h-4 w-4"
                  checked={draft.is_required}
                  onChange={(e) => {
                    create.reset()
                    setDraft({ ...draft, is_required: e.target.checked })
                  }}
                />
                {t('customFields.required')}
              </label>
              {needsOptions && options.length === 0 && (
                <p className="text-xs text-danger">{t('customFields.optionsRequired')}</p>
              )}
              <Button
                type="submit"
                className="min-h-11 justify-self-start sm:min-h-10"
                disabled={
                  create.isPending || !draft.name.trim() || (needsOptions && options.length === 0)
                }
              >
                {create.isPending ? t('customFields.adding') : t('customFields.addButton')}
              </Button>
            </form>
          </CardContent>
        </Card>
        <section className="min-w-0" aria-labelledby="configured-fields-title">
          <h2 id="configured-fields-title" className="mb-3 text-base font-semibold">
            {t('customFields.configuredTitle')}
          </h2>
          <div>
            {fields.isLoading ? (
              <p className="text-sm text-text-muted">{t('customFields.loading')}</p>
            ) : fields.error ? (
              <ErrorState
                message={t('customFields.loadError')}
                onRetry={() => void fields.refetch()}
              />
            ) : fields.data?.length ? (
              <div className="divide-y divide-border overflow-hidden rounded-md border border-border bg-surface">
                {fields.data.map((field) => (
                  <div
                    key={field.id}
                    className="flex min-h-16 items-center justify-between gap-3 p-3 hover:bg-surface-raised"
                  >
                    <div className="min-w-0 flex-1">
                      <p className="break-words font-medium">
                        {field.name}
                        {field.is_required ? ' *' : ''}
                      </p>
                      <p className="break-words text-sm text-text-muted">
                        {t(`customFields.types.${field.field_type}`)}
                        {field.options.length ? `: ${field.options.join(', ')}` : ''}
                      </p>
                    </div>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="min-h-11 min-w-11 shrink-0 text-danger hover:text-danger sm:min-h-10 sm:min-w-10"
                      aria-label={t('customFields.deleteField', { name: field.name })}
                      title={t('customFields.deleteField', { name: field.name })}
                      onClick={() => {
                        remove.reset()
                        setPendingDelete({ id: field.id, name: field.name })
                      }}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-text-muted">{t('customFields.noFields')}</p>
            )}
          </div>
        </section>
      </div>
      <ConfirmDialog
        open={pendingDelete !== null}
        onOpenChange={(open) => {
          if (!open) {
            setPendingDelete(null)
            remove.reset()
          }
        }}
        isPending={remove.isPending}
        error={remove.error ? t('customFields.deleteError') : null}
        title={t('customFields.deleteTitle')}
        description={t('customFields.deleteConfirm', { name: pendingDelete?.name })}
        onConfirm={() => {
          if (pendingDelete) {
            const { id, name } = pendingDelete
            remove.mutate(id, {
              onSuccess: () => {
                setPendingDelete(null)
                toast.success(t('customFields.deleted', { name }))
              },
            })
          }
        }}
      />
    </div>
  )
}
