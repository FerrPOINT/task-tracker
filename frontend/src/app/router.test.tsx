import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider, type RouteObject } from 'react-router'

const captured = vi.hoisted(() => ({ routes: [] as RouteObject[] }))
vi.mock('@pdlc/ui/platform', () => ({
  createProductRouter: (routes: RouteObject[]) => {
    captured.routes = routes
    return undefined
  },
  PlatformRoute: ({ children }: { children: React.ReactNode }) => children,
}))
vi.mock('@/pages/namespace', () => ({ NamespacePage: () => null }))
vi.mock('@/widgets/app-shell', () => ({ AppShell: () => null }))
vi.mock('@/shared/auth/require-auth', () => ({ RequireAuth: () => null }))
import './router'

afterEach(cleanup)

it('replaces legacy registration with login without mounting a local form or starting SSO', async () => {
  // Exercise the actual product route; substitute only the login destination
  // so this routing regression cannot initiate a real authentication flow.
  const router = createMemoryRouter(
    [
      { path: '/previous', element: <h1>Previous page</h1> },
      ...captured.routes.map((route) =>
        route.path === '/login' ? { ...route, element: <h1>Central login</h1> } : route,
      ),
    ],
    { initialEntries: ['/previous', '/register'], initialIndex: 1 },
  )
  try {
    render(<RouterProvider router={router} />)
    expect(await screen.findByRole('heading', { name: 'Central login' })).toBeInTheDocument()
    expect(router.state.location.pathname).toBe('/login')
    expect(router.state.historyAction).toBe('REPLACE')
    expect(screen.queryByRole('textbox')).toBeNull()
    await act(async () => {
      await router.navigate(-1)
    })
    await waitFor(() => expect(router.state.location.pathname).toBe('/previous'))
    expect(screen.getByRole('heading', { name: 'Previous page' })).toBeInTheDocument()
  } finally {
    router.dispose()
  }
})
