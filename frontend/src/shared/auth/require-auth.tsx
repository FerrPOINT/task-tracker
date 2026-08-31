import { useEffect, useState } from 'react'
import { Navigate, Outlet, useLocation } from 'react-router'
import { refreshAccessToken } from '@/api/client'
import { useAuthStore } from '@/shared/auth/store'

const publicPaths = ['/login', '/register']

export function RequireAuth() {
  const token = useAuthStore((s) => s.token)
  const location = useLocation()
  const isPublicPath = publicPaths.includes(location.pathname)
  const [refreshChecked, setRefreshChecked] = useState(false)

  useEffect(() => {
    if (token || isPublicPath || refreshChecked) return

    let active = true
    refreshAccessToken().finally(() => {
      if (active) setRefreshChecked(true)
    })
    return () => {
      active = false
    }
  }, [isPublicPath, refreshChecked, token])

  if (!token && !isPublicPath && refreshChecked) {
    return <Navigate to="/login" state={{ from: location }} replace />
  }

  if (!token && !isPublicPath) return null

  return <Outlet />
}
