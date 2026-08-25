import { useState } from 'react'
import { useParams } from 'react-router'
import { Button } from '@/shared/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/shared/ui/card'
import { useCreateCustomField, useDeleteCustomField, useProjectCustomFields } from '@/shared/api/hooks'
import type { CustomFieldInput, CustomFieldType } from '@/api/custom-fields'

const types: CustomFieldType[] = ['text', 'number', 'select', 'multi-select', 'date']
const initial: CustomFieldInput = { name: '', field_type: 'text', options: [], is_required: false }

export function ProjectCustomFieldsPage() {
  const { projectKey = '' } = useParams()
  const [draft, setDraft] = useState<CustomFieldInput>(initial)
  const fields = useProjectCustomFields(projectKey)
  const create = useCreateCustomField(projectKey)
  const remove = useDeleteCustomField(projectKey)
  const needsOptions = draft.field_type === 'select' || draft.field_type === 'multi-select'
  return <main className="mx-auto max-w-3xl space-y-6 p-4 md:p-6"><div><h1 className="text-2xl font-semibold">Custom fields</h1><p className="text-sm text-text-muted">Configure fields that appear on issues in {projectKey}.</p></div><Card><CardHeader><CardTitle className="text-base">Add custom field</CardTitle></CardHeader><CardContent><form className="grid gap-3" onSubmit={(e) => { e.preventDefault(); if (!draft.name.trim()) return; create.mutate(draft, { onSuccess: () => setDraft(initial) }) }}><input aria-label="Field name" className="rounded border border-border bg-background p-2" placeholder="Field name" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })}/><select aria-label="Field type" className="rounded border border-border bg-background p-2" value={draft.field_type} onChange={(e) => setDraft({ ...draft, field_type: e.target.value as CustomFieldType })}>{types.map((type) => <option key={type}>{type}</option>)}</select>{needsOptions && <input aria-label="Options" className="rounded border border-border bg-background p-2" placeholder="Options, separated by commas" value={draft.options.join(', ')} onChange={(e) => setDraft({ ...draft, options: e.target.value.split(',').map((v) => v.trim()).filter(Boolean) })}/>}<label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={draft.is_required} onChange={(e) => setDraft({ ...draft, is_required: e.target.checked })}/>Required</label><Button type="submit" disabled={create.isPending}>{create.isPending ? 'Adding...' : 'Add field'}</Button></form></CardContent></Card><Card><CardHeader><CardTitle className="text-base">Configured fields</CardTitle></CardHeader><CardContent>{fields.isLoading ? <p>Loading...</p> : fields.data?.length ? <div className="space-y-2">{fields.data.map((field) => <div key={field.id} className="flex items-center justify-between rounded border border-border p-3"><div><p className="font-medium">{field.name}{field.is_required ? ' *' : ''}</p><p className="text-sm text-text-muted">{field.field_type}{field.options.length ? `: ${field.options.join(', ')}` : ''}</p></div><Button variant="secondary" size="sm" onClick={() => remove.mutate(field.id)}>Delete</Button></div>)}</div> : <p className="text-sm text-text-muted">No custom fields configured.</p>}</CardContent></Card></main>
}
