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
    localStorage.removeItem('tt-refresh-token')
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
    const state = parsed.state ?? parsed
    return {
      token: null,
      userId: state.userId ?? state.user_id ?? null,
      email: state.email ?? null,
      username: state.username ?? null,
      displayName: state.displayName ?? state.display_name ?? null,
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

// The refresh token lives ONLY in the HttpOnly cookie set by the backend.
// It is never copied into localStorage: an XSS payload must not be able to
// read it and silently extend the session (audit r4, P1).
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
        set({
          token: null,
          userId: null,
          email: null,
          username: null,
          displayName: null,
        })
      },
    }),
    {
      name: 'task-tracker-auth',
      merge: (persistedState, currentState) => {
        const persisted = (persistedState ?? {}) as Partial<AuthState>
        return {
          ...currentState,
          token: null,
          userId: persisted.userId ?? currentState.userId,
          email: persisted.email ?? currentState.email,
          username: persisted.username ?? currentState.username,
          displayName: persisted.displayName ?? currentState.displayName,
        }
      },
      partialize: (state) => ({
        userId: state.userId,
        email: state.email,
        username: state.username,
        displayName: state.displayName,
      }),
    },
  ),
)
