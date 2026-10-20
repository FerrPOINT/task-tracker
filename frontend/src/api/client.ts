import createClient from 'openapi-fetch'
import type { paths } from './generated'
import { useAuthStore } from '@/shared/auth/store'
import { getPlatformProduct } from '@pdlc/ui/platform'

export const apiBaseUrl =
  import.meta.env.VITE_API_BASE_URL?.replace('/api/v1', '') ??
  import.meta.env.BASE_URL.replace(/\/$/, '')

export const api = createClient<paths>({ baseUrl: apiBaseUrl, credentials: 'include' })

export async function refreshAccessToken(): Promise<boolean> {
  const product = getPlatformProduct()
  if (product) {
    product.recheckAuthorization()
    return false
  }
  useAuthStore.getState().logout()
  window.location.assign(`${import.meta.env.BASE_URL}login`)
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
      const token = useAuthStore.getState().token
      const current = token ? 'Bearer ' + token : null
      // A late failure from a previous session must not log out its successor.
      if (request.headers.get('authorization') === current) void refreshAccessToken()
    }
    return response
  },
})

export type * from './generated'
