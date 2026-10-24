import { apiBaseUrl, refreshAccessToken } from './client'
import { useAuthStore } from '@/shared/auth/store'

export type IssueExportFormat = 'csv' | 'json'

export interface IssueExportFile {
  blob: Blob
  filename: string
}

export async function fetchIssueExport(
  projectKey: string,
  format: IssueExportFormat,
  options: { isCurrent?: () => boolean } = {},
): Promise<IssueExportFile> {
  const initial = useAuthStore.getState()
  const isCurrent = () => options.isCurrent?.() !== false
  if (!isCurrent()) throw new Error('export context changed')
  const request = (token: string | null) =>
    fetch(`${apiBaseUrl}/api/v1/export/${format}`, {
      method: 'POST',
      credentials: 'include',
      headers: { 'Content-Type': 'application/json', Authorization: 'Bearer ' + (token ?? '') },
      body: JSON.stringify({ project_key: projectKey }),
    })
  let response = await request(initial.token)
  const beforeRefresh = useAuthStore.getState()
  if (
    response.status === 401 &&
    isCurrent() &&
    initial.userId &&
    beforeRefresh.userId === initial.userId &&
    beforeRefresh.token === initial.token
  ) {
    const refreshed = await refreshAccessToken()
    const current = useAuthStore.getState()
    if (refreshed && isCurrent() && current.userId === initial.userId && current.token)
      response = await request(current.token)
  }
  if (!isCurrent()) throw new Error('export context changed')
  if (!response.ok) {
    throw new Error('failed to export issues')
  }

  const disposition = response.headers.get('content-disposition') ?? ''
  const filename = disposition.match(/filename="?([^";]+)"?/)?.[1] ?? `issues.${format}`
  const blob = await response.blob()
  if (!isCurrent()) throw new Error('export context changed')
  return { blob, filename }
}
