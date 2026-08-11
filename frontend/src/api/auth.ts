import { api } from './client'
import type { components } from './generated'

export type LoginRequest = components['schemas']['LoginRequest']
export type RegisterRequest = components['schemas']['RegisterRequest']
export type AuthResponse = components['schemas']['AuthResponse']
export type UserResponse = components['schemas']['UserResponse']

export async function login(req: LoginRequest): Promise<AuthResponse> {
  const { data, error } = await api.POST('/api/v1/auth/login', { body: req })
  if (error || !data) throw new Error('failed to login')
  return data
}

export async function register(req: RegisterRequest): Promise<AuthResponse> {
  const { data, error } = await api.POST('/api/v1/auth/register', { body: req })
  if (error || !data) throw new Error('failed to register')
  return data
}

export type User = components['schemas']['UserResponse']

export async function listUsers(): Promise<User[]> {
  const { data, error } = await api.GET('/api/v1/users')
  if (error || !data) throw new Error('failed to load users')
  return data.users
}

export async function getCurrentUser(): Promise<UserResponse> {
  const { data, error } = await api.GET('/api/v1/users/me')
  if (error || !data) throw new Error('failed to load current user')
  return data
}

export async function logout(): Promise<void> {
  const { error } = await api.POST('/api/v1/auth/logout')
  if (error) throw new Error('failed to logout')
}
