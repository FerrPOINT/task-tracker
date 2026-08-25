import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useIssueCustomFieldValues, useProjectCustomFields, useSetIssueCustomFieldValue } from '@/shared/api/hooks'
import type { CustomField } from '@/api/custom-fields'

function FieldInput({ field, value, onSave }: { field: CustomField; value: unknown; onSave: (value: unknown) => void }) {
  const { t } = useTranslation()
  const current = typeof value === 'string' ? value : ''
  if (field.field_type === 'select') {
    return (
      <select className="w-full rounded border border-border bg-background p-2" value={current} onChange={(e) => onSave(e.target.value)}>
        <option value="">{t('customFields.selectPlaceholder')}</option>
        {field.options.map((option) => <option key={option} value={option}>{option}</option>)}
      </select>
    )
  }
  if (field.field_type === 'multi-select') {
    const selected = Array.isArray(value) ? value.map(String) : []
    return <select multiple className="w-full rounded border border-border bg-background p-2" value={selected} onChange={(e) => onSave(Array.from(e.target.selectedOptions, (o) => o.value))}>{field.options.map((option) => <option key={option} value={option}>{option}</option>)}</select>
  }
  return <input className="w-full rounded border border-border bg-background p-2" type={field.field_type === 'number' ? 'number' : field.field_type === 'date' ? 'date' : 'text'} defaultValue={current} onBlur={(e) => onSave(field.field_type === 'number' && e.target.value ? Number(e.target.value) : e.target.value)} />
}

export function CustomFieldsPanel({ issueId, projectKey }: { issueId: string; projectKey: string }) {
  const { t } = useTranslation()
  const fields = useProjectCustomFields(projectKey)
  const values = useIssueCustomFieldValues(issueId)
  const save = useSetIssueCustomFieldValue(issueId)
  const byField = useMemo(() => new Map((values.data ?? []).map((v) => [v.field_id, v.value])), [values.data])
  if (!fields.data?.length) return <p className="text-sm text-text-muted">{t('customFields.noFields')}</p>
  return <div className="space-y-3">{fields.data.map((field) => <label key={field.id} className="block text-sm font-medium text-text-secondary">{field.name}{field.is_required ? ' *' : ''}<div className="mt-1"><FieldInput field={field} value={byField.get(field.id)} onSave={(value) => save.mutate({ fieldId: field.id, value })} /></div></label>)}</div>
}