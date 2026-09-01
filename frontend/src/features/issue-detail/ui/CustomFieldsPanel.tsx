import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  useIssueCustomFieldValues,
  useProjectCustomFields,
  useSetIssueCustomFieldValue,
} from '@/shared/api/hooks'
import type { CustomField } from '@/api/custom-fields'

export function customFieldDateInputValue(value: unknown) {
  if (typeof value !== 'string') return ''
  if (/^\d{4}-\d{2}-\d{2}(?:$|T)/.test(value)) return value.slice(0, 10)
  return ''
}

export function isEmptyCustomFieldValue(value: unknown) {
  return (
    value == null ||
    (typeof value === 'string' && value.trim() === '') ||
    (Array.isArray(value) && value.length === 0)
  )
}

function customFieldInputValue(field: CustomField, value: unknown) {
  if (field.field_type === 'date') return customFieldDateInputValue(value)
  if (typeof value === 'number') return String(value)
  if (typeof value === 'string') return value
  return ''
}

function customFieldValueFromInput(field: CustomField, value: string) {
  if (field.field_type === 'number' && value !== '') return Number(value)
  return value
}

export function CustomFieldValueInput({
  field,
  value,
  onSave,
  commit = 'blur',
}: {
  field: CustomField
  value: unknown
  onSave: (value: unknown) => void
  commit?: 'blur' | 'change'
}) {
  const { t } = useTranslation()
  const current = customFieldInputValue(field, value)
  const [draft, setDraft] = useState(current)

  useEffect(() => {
    setDraft(current)
  }, [current])

  if (field.field_type === 'select') {
    return (
      <select
        className="w-full rounded border border-border bg-background p-2"
        value={current}
        onChange={(e) => onSave(e.target.value)}
      >
        <option value="">{t('customFields.selectPlaceholder')}</option>
        {field.options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
    )
  }
  if (field.field_type === 'multi-select') {
    const selected = Array.isArray(value) ? value.map(String) : []
    return (
      <select
        multiple
        className="w-full rounded border border-border bg-background p-2"
        value={selected}
        onChange={(e) => onSave(Array.from(e.target.selectedOptions, (o) => o.value))}
      >
        {field.options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
    )
  }
  const type =
    field.field_type === 'number' ? 'number' : field.field_type === 'date' ? 'date' : 'text'
  if (commit === 'change') {
    return (
      <input
        className="w-full rounded border border-border bg-background p-2"
        type={type}
        value={current}
        onChange={(e) => onSave(customFieldValueFromInput(field, e.target.value))}
      />
    )
  }
  return (
    <input
      className="w-full rounded border border-border bg-background p-2"
      type={type}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => onSave(customFieldValueFromInput(field, draft))}
    />
  )
}

export function CustomFieldsPanel({
  issueId,
  projectKey,
}: {
  issueId: string
  projectKey: string
}) {
  const { t } = useTranslation()
  const fields = useProjectCustomFields(projectKey)
  const values = useIssueCustomFieldValues(issueId)
  const save = useSetIssueCustomFieldValue(issueId)
  const byField = useMemo(
    () => new Map((values.data ?? []).map((v) => [v.field_id, v.value])),
    [values.data],
  )
  if (!fields.data?.length)
    return <p className="text-sm text-text-muted">{t('customFields.noFields')}</p>
  return (
    <div className="space-y-3">
      {fields.data.map((field) => (
        <label key={field.id} className="block text-sm font-medium text-text-secondary">
          {field.name}
          {field.is_required ? ' *' : ''}
          <div className="mt-1">
            <CustomFieldValueInput
              field={field}
              value={byField.get(field.id)}
              onSave={(value) => save.mutate({ fieldId: field.id, value })}
            />
          </div>
        </label>
      ))}
    </div>
  )
}
