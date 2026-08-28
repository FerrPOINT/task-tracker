import createClient from 'openapi-fetch'
import type { paths } from './generated'
import { useAuthStore } from '@/shared/auth/store'

const baseUrl = import.meta.env.VITE_API_BASE_URL?.replace('/api/v1', '') ?? ''

export const api = createClient<paths>({ baseUrl, credentials: 'include' })

let refreshPromise: Promise<boolean> | null = null

async function refreshAccessToken(): Promise<boolean> {
  if (refreshPromise) return refreshPromise
  refreshPromise = (async () => {
    try {
      // The HttpOnly refresh cookie (same-origin via nginx/vite proxy) is
      // the only refresh credential the browser holds.
      const res = await fetch(`${baseUrl}/api/v1/auth/refresh`, {
        method: 'POST',
        credentials: 'include',
        headers: { 'Content-Type': 'application/json' },
        body: '{}',
      })
      if (!res.ok) {
        useAuthStore.getState().logout()
        window.location.href = '/login'
        return false
      }
      const data = (await res.json()) as { access_token?: string }
      if (data.access_token) {
        useAuthStore.setState({ token: data.access_token })
      }
      return true
    } catch {
      useAuthStore.getState().logout()
      window.location.href = '/login'
      return false
    } finally {
      refreshPromise = null
    }
  })()
  return refreshPromise
}

function shouldIntercept401(req: Request): boolean {
  const url = req.url
  return !url.includes('/api/v1/auth/')
}

api.use({
  onRequest: ({ request }) => {
    const token = useAuthStore.getState().token
    if (token) {
      request.headers.set('Authorization', `Bearer ${token}`)
    }
    return request
  },
  onResponse: async ({ request, response, options }) => {
    if (response.status !== 401 || !shouldIntercept401(request)) {
      return response
    }
    const ok = await refreshAccessToken()
    if (!ok) return response
    const token = useAuthStore.getState().token
    const nextRequest = new Request(request)
    if (token) {
      nextRequest.headers.set('Authorization', `Bearer ${token}`)
    }
    return fetch(nextRequest, options as unknown as RequestInit)
  },
})

export type * from './generated'
