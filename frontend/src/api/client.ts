import createClient from 'openapi-fetch'
import type { paths } from './generated'
import { useAuthStore } from '@/shared/auth/store'

const baseUrl = import.meta.env.VITE_API_BASE_URL?.replace('/api/v1', '') ?? ''

export const api = createClient<paths>({ baseUrl })

api.use({
  onRequest: ({ request }) => {
    const token = useAuthStore.getState().token
    if (token) {
      request.headers.set('Authorization', `Bearer ${token}`)
    }
    return request
  },
})

export type * from './generated'
