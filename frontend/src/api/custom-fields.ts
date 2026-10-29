import { api } from './client'
import { ApiError, InvalidResponseError } from '@pdlc/ui/lib'

export type CustomFieldType = 'text' | 'number' | 'select' | 'multi-select' | 'date'
export interface CustomField {
  id: string
  project_id: string
  name: string
  field_type: CustomFieldType
  options: string[]
  is_required: boolean
  created_at: string
}
export interface CustomFieldInput {
  name: string
  field_type: CustomFieldType
  options: string[]
  is_required: boolean
}
export interface CustomFieldValue {
  field_id: string
  value: unknown
}

export async function listCustomFields(projectKey: string): Promise<CustomField[]> {
  const { data, error, response } = await api.GET('/api/v1/projects/{project_key}/custom-fields', {
    params: { path: { project_key: projectKey } },
  })
  if (error || !response.ok) throw new ApiError(response.status, 'Failed to load custom fields')
  if (!data || !Array.isArray((data as { fields?: unknown }).fields))
    throw new InvalidResponseError(response.status)
  return (data as { fields: CustomField[] }).fields
}
export async function createCustomField(
  projectKey: string,
  input: CustomFieldInput,
): Promise<CustomField> {
  const { data, error } = await api.POST(
    '/api/v1/projects/{project_key}/custom-fields' as never,
    { params: { path: { project_key: projectKey } }, body: input } as never,
  )
  if (error || !data) throw new Error('Failed to create custom field')
  return data as CustomField
}
export async function updateCustomField(id: string, input: CustomFieldInput): Promise<CustomField> {
  const { data, error } = await api.PUT(
    '/api/v1/custom-fields/{id}' as never,
    { params: { path: { id } }, body: input } as never,
  )
  if (error || !data) throw new Error('Failed to update custom field')
  return data as CustomField
}
export async function deleteCustomField(id: string): Promise<void> {
  const { error } = await api.DELETE(
    '/api/v1/custom-fields/{id}' as never,
    { params: { path: { id } } } as never,
  )
  if (error) throw new Error('Failed to delete custom field')
}
export async function listIssueCustomFieldValues(issueId: string): Promise<CustomFieldValue[]> {
  const { data, error, response } = await api.GET('/api/v1/issues/{issue_id}/custom-fields', {
    params: { path: { issue_id: issueId } },
  })
  if (error || !response.ok)
    throw new ApiError(response.status, 'Failed to load custom field values')
  if (!data || !Array.isArray((data as { values?: unknown }).values))
    throw new InvalidResponseError(response.status)
  return (data as { values: CustomFieldValue[] }).values
}
export async function setIssueCustomFieldValue(
  issueId: string,
  fieldId: string,
  value: unknown,
): Promise<void> {
  const { error } = await api.PUT(
    '/api/v1/issues/{issue_id}/custom-fields/{field_id}/value' as never,
    { params: { path: { issue_id: issueId, field_id: fieldId } }, body: { value } } as never,
  )
  if (error) throw new Error('Failed to save custom field value')
}
