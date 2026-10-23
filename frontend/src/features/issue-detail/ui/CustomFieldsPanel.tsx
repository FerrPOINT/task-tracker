import { MultiSelect, Input, Select, FormField, toast } from '@pdlc/ui/ui'

import { useEffect, useId, useMemo, useState } from 'react'
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
  id,
  field,
  value,
  onSave,
  commit = 'blur',
}: {
  id?: string
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
      <Select
        id={id}
        className="w-full"
        value={current}
        onValueChange={(nextValue) => onSave(nextValue)}
        options={[
          { value: '', label: String(t('customFields.selectPlaceholder')) },
          ...(field.options.map((option) => ({ value: String(option), label: String(option) })) ??
            []),
        ]}
      />
    )
  }
  if (field.field_type === 'multi-select') {
    const selected = Array.isArray(value) ? value.map(String) : []
    return (
      <MultiSelect
        id={id}
        className="w-full"
        value={selected}
        onValueChange={(nextValues) => onSave(nextValues)}
        options={[
          ...(field.options.map((option) => ({ value: String(option), label: String(option) })) ??
            []),
        ]}
      />
    )
  }
  const type =
    field.field_type === 'number' ? 'number' : field.field_type === 'date' ? 'date' : 'text'
  if (commit === 'change') {
    return (
      <Input
        id={id}
        type={type}
        value={current}
        onChange={(e) => onSave(customFieldValueFromInput(field, e.target.value))}
      />
    )
  }
  return (
    <Input
      id={id}
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
  const fieldId = useId()
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
        <FormField
          key={field.id}
          id={`${fieldId}-${field.id}`}
          label={field.name}
          required={field.is_required}
        >
          {({ id }) => (
            <CustomFieldValueInput
              id={id}
              field={field}
              value={byField.get(field.id)}
              onSave={(value) =>
                save.mutate(
                  { fieldId: field.id, value },
                  {
                    onSuccess: () => toast.success(t('common.saved')),
                    onError: (error) => toast.error(error.message),
                  },
                )
              }
            />
          )}
        </FormField>
      ))}
    </div>
  )
}
