import { Navigate, Outlet, useLocation } from 'react-router'
import { useAuthStore } from '@/shared/auth/store'

const publicPaths = ['/login', '/register']

export function RequireAuth() {
  const token = useAuthStore((s) => s.token)
  const location = useLocation()

  if (!token && !publicPaths.includes(location.pathname)) {
    return <Navigate to="/login" state={{ from: location }} replace />
  }

  return <Outlet />
}
