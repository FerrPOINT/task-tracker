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
): Promise<IssueExportFile> {
  const request = () =>
    fetch(`${apiBaseUrl}/api/v1/export/${format}`, {
      method: 'POST',
      credentials: 'include',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${useAuthStore.getState().token ?? ''}`,
      },
      body: JSON.stringify({ project_key: projectKey }),
    })

  let response = await request()
  if (response.status === 401 && (await refreshAccessToken())) {
    response = await request()
  }
  if (!response.ok) {
    throw new Error('failed to export issues')
  }

  const disposition = response.headers.get('content-disposition') ?? ''
  const filename = disposition.match(/filename="?([^";]+)"?/)?.[1] ?? `issues.${format}`
  return { blob: await response.blob(), filename }
}
