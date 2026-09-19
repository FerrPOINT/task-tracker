import createClient from 'openapi-fetch'
import type { paths } from './generated'
import { useAuthStore } from '@/shared/auth/store'

export const apiBaseUrl = import.meta.env.VITE_API_BASE_URL?.replace('/api/v1', '') ?? ''

export const api = createClient<paths>({ baseUrl: apiBaseUrl, credentials: 'include' })

export async function refreshAccessToken(): Promise<boolean> {
  useAuthStore.getState().logout()
  window.location.assign('/login')
  return false
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
  onResponse: ({ request, response }) => {
    if (response.status === 401 && shouldIntercept401(request)) {
      void refreshAccessToken()
    }
    return response
  },
})

export type * from './generated'
