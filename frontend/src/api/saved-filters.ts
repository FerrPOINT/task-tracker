import { api } from './client'
import type { components } from './generated'

export type SavedFilter = components['schemas']['SavedFilterResponse']

export interface CreateSavedFilterInput {
  name: string
  jql: string
  is_public?: boolean
}

export async function listSavedFilters(): Promise<SavedFilter[]> {
  const { data } = await api.GET('/api/v1/filters', {})
  if (!data) throw new Error('Failed to list filters')
  return data.filters ?? []
}

export async function createSavedFilter(
  input: CreateSavedFilterInput,
): Promise<SavedFilter> {
  const { data, error } = await api.POST('/api/v1/filters', {
    body: {
      name: input.name,
      jql: input.jql,
      is_public: input.is_public ?? false,
    },
  })
  if (!data) throw new Error(error ? JSON.stringify(error) : 'Failed to create filter')
  return data
}

export async function getSavedFilter(id: string): Promise<SavedFilter> {
  const { data } = await api.GET('/api/v1/filters/{id}', {
    params: { path: { id } },
  })
  if (!data) throw new Error('Failed to get filter')
  return data
}

export async function deleteSavedFilter(id: string): Promise<void> {
  await api.DELETE('/api/v1/filters/{id}', {
    params: { path: { id } },
  })
}

export async function executeSavedFilter(id: string): Promise<unknown[]> {
  const { data } = await api.GET('/api/v1/filters/{id}/execute', {
    params: { path: { id } },
  })
  if (!data) throw new Error('Failed to execute filter')
  return data.issues ?? []
}