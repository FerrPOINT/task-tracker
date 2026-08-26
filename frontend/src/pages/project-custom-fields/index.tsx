import { useState } from 'react'
import { useParams } from 'react-router'
import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/shared/ui/card'
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
  const fields = useProjectCustomFields(projectKey)
  const create = useCreateCustomField(projectKey)
  const remove = useDeleteCustomField(projectKey)
  const needsOptions = draft.field_type === 'select' || draft.field_type === 'multi-select'
  return (
    <main className="mx-auto max-w-3xl space-y-6 p-4 md:p-6">
      <div>
        <h1 className="text-2xl font-semibold">{t('customFields.title')}</h1>
        <p className="text-sm text-text-muted">{t('customFields.description', { projectKey })}</p>
      </div>
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
              if (needsOptions && draft.options.length === 0) return
              create.mutate(draft, { onSuccess: () => setDraft(initial) })
            }}
          >
            <input
              aria-label={t('customFields.fieldName')}
              className="rounded border border-border bg-background p-2"
              placeholder={t('customFields.fieldName')}
              value={draft.name}
              onChange={(e) => setDraft({ ...draft, name: e.target.value })}
            />
            <select
              aria-label={t('customFields.fieldType')}
              className="rounded border border-border bg-background p-2"
              value={draft.field_type}
              onChange={(e) =>
                setDraft({ ...draft, field_type: e.target.value as CustomFieldType })
              }
            >
              {types.map((type) => (
                <option key={type}>{type}</option>
              ))}
            </select>
            {needsOptions && (
              <input
                aria-label={t('customFields.options')}
                className="rounded border border-border bg-background p-2"
                placeholder={t('customFields.options')}
                value={draft.options.join(', ')}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    options: e.target.value
                      .split(',')
                      .map((v) => v.trim())
                      .filter(Boolean),
                  })
                }
              />
            )}
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={draft.is_required}
                onChange={(e) => setDraft({ ...draft, is_required: e.target.checked })}
              />
              {t('customFields.required')}
            </label>
            {needsOptions && draft.options.length === 0 && (
              <p className="text-xs text-danger">{t('customFields.optionsRequired')}</p>
            )}
            <Button
              type="submit"
              disabled={create.isPending || (needsOptions && draft.options.length === 0)}
            >
              {create.isPending ? t('customFields.adding') : t('customFields.addButton')}
            </Button>
          </form>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle className="text-base">{t('customFields.configuredTitle')}</CardTitle>
        </CardHeader>
        <CardContent>
          {fields.isLoading ? (
            <p>{t('customFields.loading')}</p>
          ) : fields.data?.length ? (
            <div className="space-y-2">
              {fields.data.map((field) => (
                <div
                  key={field.id}
                  className="flex items-center justify-between rounded border border-border p-3"
                >
                  <div>
                    <p className="font-medium">
                      {field.name}
                      {field.is_required ? ' *' : ''}
                    </p>
                    <p className="text-sm text-text-muted">
                      {field.field_type}
                      {field.options.length ? `: ${field.options.join(', ')}` : ''}
                    </p>
                  </div>
                  <Button variant="secondary" size="sm" onClick={() => remove.mutate(field.id)}>
                    {t('common.delete')}
                  </Button>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-sm text-text-muted">{t('customFields.noFields')}</p>
          )}
        </CardContent>
      </Card>
    </main>
  )
}
