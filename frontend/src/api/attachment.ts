import { api } from './client'
import type { components } from './generated'

export type Attachment = components['schemas']['AttachmentResponse']

export async function listAttachments(issueId: string): Promise<Attachment[]> {
  const { data, error } = await api.GET('/api/v1/issues/{issue_id}/attachments', {
    params: { path: { issue_id: issueId } },
  })
  if (error || !data) throw new Error('Failed to load attachments')
  return data.attachments
}

function authHeader(): Record<string, string> {
  const token = JSON.parse(localStorage.getItem('task-tracker-auth') ?? '{}')?.state?.token ?? ''
  return token ? { Authorization: `Bearer ${token}` } : {}
}

export async function uploadAttachment(
  issueId: string,
  file: File,
  onProgress?: (loaded: number, total: number) => void,
): Promise<Attachment> {
  return new Promise((resolve, reject) => {
    const form = new FormData()
    form.append('file', file)
    const xhr = new XMLHttpRequest()
    xhr.open('POST', `/api/v1/issues/${issueId}/attachments`)
    const headers = authHeader()
    for (const [key, value] of Object.entries(headers)) {
      xhr.setRequestHeader(key, value)
    }
    if (onProgress && xhr.upload) {
      xhr.upload.onprogress = (e) => {
        if (e.lengthComputable) onProgress(e.loaded, e.total)
      }
    }
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        try {
          resolve(JSON.parse(xhr.responseText))
        } catch {
          reject(new Error('Invalid response'))
        }
      } else {
        try {
          const body = JSON.parse(xhr.responseText)
          reject(new Error(body.error ?? 'upload failed'))
        } catch {
          reject(new Error('upload failed'))
        }
      }
    }
    xhr.onerror = () => reject(new Error('upload failed'))
    xhr.send(form)
  })
}

export async function deleteAttachment(id: string): Promise<void> {
  const { error } = await api.DELETE('/api/v1/attachments/{id}', {
    params: { path: { id } },
  })
  if (error) throw new Error('Failed to delete attachment')
}

/** Fetches the file with auth and triggers a browser download. */
export async function downloadAttachment(a: Attachment): Promise<void> {
  const res = await fetch(`/api/v1/attachments/${a.id}/download`, {
    headers: authHeader(),
  })
  if (!res.ok) throw new Error('download failed')
  const blob = await res.blob()
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = a.file_name
  document.body.appendChild(link)
  link.click()
  link.remove()
  URL.revokeObjectURL(url)
}
