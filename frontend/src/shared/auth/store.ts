import { create } from 'zustand'
import { persist } from 'zustand/middleware'

function readStoredAuth(): { token: string | null; userId: string | null; email: string | null } {
  try {
    const raw = localStorage.getItem('task-tracker-auth')
    if (!raw) return { token: null, userId: null, email: null }
    const parsed = JSON.parse(raw)
    return {
      token: parsed.token ?? null,
      userId: parsed.userId ?? parsed.user_id ?? null,
      email: parsed.email ?? null,
    }
  } catch {
    return { token: null, userId: null, email: null }
  }
}

interface AuthState {
  token: string | null
  userId: string | null
  email: string | null
  setAuth: (token: string, userId: string, email: string) => void
  logout: () => void
}

const initial = readStoredAuth()

export const useAuthStore = create<AuthState>()(
  persist(
    (set) => ({
      token: initial.token,
      userId: initial.userId,
      email: initial.email,
      setAuth: (token, userId, email) => set({ token, userId, email }),
      logout: () => set({ token: null, userId: null, email: null }),
    }),
    { name: 'task-tracker-auth' },
  ),
)
