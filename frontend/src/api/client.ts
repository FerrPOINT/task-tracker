import createClient from 'openapi-fetch'
import type { paths } from './generated'
import { readRefreshToken, storeRefreshToken, useAuthStore } from '@/shared/auth/store'

const baseUrl = import.meta.env.VITE_API_BASE_URL?.replace('/api/v1', '') ?? ''

export const api = createClient<paths>({ baseUrl })

let refreshPromise: Promise<boolean> | null = null

async function refreshAccessToken(): Promise<boolean> {
  if (refreshPromise) return refreshPromise
  refreshPromise = (async () => {
    try {
      // On plain-HTTP deployments the Secure refresh cookie never reaches
      // the browser; send the stored refresh token as a body fallback.
      const refreshToken = readRefreshToken()
      const res = await fetch(`${baseUrl}/api/v1/auth/refresh`, {
        method: 'POST',
        credentials: 'include',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(refreshToken ? { refresh_token: refreshToken } : {}),
      })
      if (!res.ok) {
        useAuthStore.getState().logout()
        window.location.href = '/login'
        return false
      }
      const data = (await res.json()) as { access_token?: string; refresh_token?: string }
      // Cookie-first: the rotated token arrives as an HttpOnly Set-Cookie on
      // HTTPS deployments. The localStorage copy exists ONLY as a plain-HTTP
      // fallback; when the server also returns the token in the body we keep
      // the fallback fresh, otherwise we drop the stale one.
      if (data.refresh_token) {
        storeRefreshToken(data.refresh_token)
      } else {
        storeRefreshToken(null)
      }
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
