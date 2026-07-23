import createClient from 'openapi-fetch'
import type { paths } from './generated'

const baseUrl = import.meta.env.VITE_API_BASE_URL ?? 'http://127.0.0.1:3456/api/v1'

export const api = createClient<paths>({ baseUrl })

export type * from './generated'
