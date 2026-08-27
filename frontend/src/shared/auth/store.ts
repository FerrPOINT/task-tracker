import { create } from 'zustand'
import { persist } from 'zustand/middleware'

function readStoredAuth(): {
  token: string | null
  userId: string | null
  email: string | null
  username: string | null
  displayName: string | null
} {
  try {
    const raw = localStorage.getItem('task-tracker-auth')
    if (!raw)
      return {
        token: null,
        userId: null,
        email: null,
        username: null,
        displayName: null,
      }
    const parsed = JSON.parse(raw)
    return {
      token: parsed.token ?? null,
      userId: parsed.userId ?? parsed.user_id ?? null,
      email: parsed.email ?? null,
      username: parsed.username ?? null,
      displayName: parsed.displayName ?? parsed.display_name ?? null,
    }
  } catch {
    return {
      token: null,
      userId: null,
      email: null,
      username: null,
      displayName: null,
    }
  }
}

const REFRESH_KEY = 'tt-refresh-token'

export function storeRefreshToken(token: string | null): void {
  try {
    if (token) localStorage.setItem(REFRESH_KEY, token)
    else localStorage.removeItem(REFRESH_KEY)
  } catch {
    // storage unavailable — cookie flow remains
  }
}

export function readRefreshToken(): string | null {
  try {
    return localStorage.getItem(REFRESH_KEY)
  } catch {
    return null
  }
}

interface AuthState {
  token: string | null
  userId: string | null
  email: string | null
  username: string | null
  displayName: string | null
  setAuth: (payload: {
    token: string
    userId: string
    email: string
    username?: string
    displayName?: string
  }) => void
  setUser: (payload: {
    userId?: string
    email?: string
    username?: string
    displayName?: string
  }) => void
  logout: () => void
}

const initial = readStoredAuth()

export const useAuthStore = create<AuthState>()(
  persist(
    (set) => ({
      token: initial.token,
      userId: initial.userId,
      email: initial.email,
      username: initial.username,
      displayName: initial.displayName,
      setAuth: (payload) =>
        set({
          token: payload.token,
          userId: payload.userId,
          email: payload.email,
          username: payload.username ?? null,
          displayName: payload.displayName ?? null,
        }),
      setUser: (payload) =>
        set((state) => ({
          userId: payload.userId ?? state.userId,
          email: payload.email ?? state.email,
          username: payload.username ?? state.username,
          displayName: payload.displayName ?? state.displayName,
        })),
      logout: () => {
        storeRefreshToken(null)
        set({
          token: null,
          userId: null,
          email: null,
          username: null,
          displayName: null,
        })
      },
    }),
    { name: 'task-tracker-auth' },
  ),
)
