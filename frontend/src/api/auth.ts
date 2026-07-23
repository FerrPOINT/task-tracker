import { api } from './client'
import type { components } from './generated'

export type LoginRequest = components['schemas']['LoginRequest']
export type RegisterRequest = components['schemas']['RegisterRequest']
export type AuthResponse = components['schemas']['AuthResponse']

export async function login(req: LoginRequest): Promise<AuthResponse> {
  const { data, error } = await api.POST('/auth/login', { body: req })
  if (error || !data) throw new Error('failed to login')
  return data
}

export async function register(req: RegisterRequest): Promise<AuthResponse> {
  const { data, error } = await api.POST('/auth/register', { body: req })
  if (error || !data) throw new Error('failed to register')
  return data
}
