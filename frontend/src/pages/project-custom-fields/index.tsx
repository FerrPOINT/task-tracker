import { useState } from 'react'
import { useParams } from 'react-router'
import { useTranslation } from 'react-i18next'
import {
  SectionHeader,
  toast,
  Input,
  Select,
  Button,
  ConfirmDialog,
  ErrorState,
  ResourceList,
  ResourceListRow,
  Checkbox,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Notice,
} from '@pdlc/ui/ui'
import { Trash2 } from 'lucide-react'

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
  const [adding, setAdding] = useState(false)
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
      <SectionHeader
        level={2}
        title={<>{t('customFields.title')}</>}
        actions={
          <>
            {!adding && (
              <Button onClick={() => setAdding(true)}>{t('customFields.addButton')}</Button>
            )}
          </>
        }
      />
      <div className="space-y-5">
        {adding && (
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
                        setAdding(false)
                        setDraft(initial)
                        setOptionsText('')
                        toast.success(t('customFields.created'))
                      },
                    },
                  )
                }}
              >
                {create.error && (
                  <Notice role="alert" tone="danger" appearance="inline">
                    {t('customFields.createError')}
                  </Notice>
                )}
                <Input
                  aria-label={t('customFields.fieldName')}
                  placeholder={t('customFields.fieldName')}
                  required
                  value={draft.name}
                  onChange={(e) => {
                    create.reset()
                    setDraft({ ...draft, name: e.target.value })
                  }}
                />
                <Select
                  aria-label={t('customFields.fieldType')}
                  className="w-auto"
                  value={draft.field_type}
                  onValueChange={(nextValue) => {
                    create.reset()
                    setDraft({ ...draft, field_type: nextValue as CustomFieldType })
                  }}
                  options={[
                    ...(types.map((type) => ({
                      value: String(type),
                      label: String(t(`customFields.types.${type}`)),
                    })) ?? []),
                  ]}
                />
                {needsOptions && (
                  <Input
                    aria-label={t('customFields.options')}
                    placeholder={t('customFields.options')}
                    value={optionsText}
                    onChange={(e) => {
                      create.reset()
                      setOptionsText(e.target.value)
                    }}
                  />
                )}
                <Checkbox
                  checked={draft.is_required}
                  onCheckedChange={(checked) => {
                    create.reset()
                    setDraft({ ...draft, is_required: checked === true })
                  }}
                >
                  {t('customFields.required')}
                </Checkbox>
                {needsOptions && options.length === 0 && (
                  <p className="text-xs text-danger">{t('customFields.optionsRequired')}</p>
                )}
                <div className="flex gap-2">
                  <Button
                    type="submit"
                    className="justify-self-start"
                    disabled={
                      create.isPending ||
                      !draft.name.trim() ||
                      (needsOptions && options.length === 0)
                    }
                  >
                    {create.isPending ? t('customFields.adding') : t('customFields.addButton')}
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    disabled={create.isPending}
                    onClick={() => {
                      setAdding(false)
                      setDraft(initial)
                      setOptionsText('')
                      create.reset()
                    }}
                  >
                    {t('common.cancel')}
                  </Button>
                </div>
              </form>
            </CardContent>
          </Card>
        )}
        <section className="min-w-0" aria-labelledby="configured-fields-title">
          <SectionHeader
            level={2}
            headingId="configured-fields-title"
            className="mb-3"
            title={<>{t('customFields.configuredTitle')}</>}
          />
          <div>
            {fields.isLoading ? (
              <p className="text-sm text-text-muted">{t('customFields.loading')}</p>
            ) : fields.error ? (
              <ErrorState
                message={t('customFields.loadError')}
                onRetry={() => void fields.refetch()}
              />
            ) : fields.data?.length ? (
              <ResourceList asChild variant="bordered">
                <div className="overflow-hidden">
                  {fields.data.map((field) => (
                    <ResourceListRow asChild key={field.id}>
                      <div className="flex items-center justify-between gap-3">
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
                          variant="destructive-outline"
                          size="icon"
                          className="shrink-0"
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
                    </ResourceListRow>
                  ))}
                </div>
              </ResourceList>
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
