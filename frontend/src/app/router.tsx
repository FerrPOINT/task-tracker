import { lazy, Suspense } from 'react'
import { createBrowserRouter, Navigate } from 'react-router'
import { AppShell } from '@/widgets/app-shell'
import { RequireAuth } from '@/shared/auth/require-auth'

// Route-level code splitting: all pages are lazy-loaded so each route
// only downloads the code it actually needs.
const DashboardPage = lazy(() =>
  import('@/pages/dashboard').then((m) => ({ default: m.DashboardPage })),
)
const ProjectsPage = lazy(() =>
  import('@/pages/projects').then((m) => ({ default: m.ProjectsPage })),
)
const ProjectBoardPage = lazy(() =>
  import('@/pages/project-board').then((m) => ({ default: m.ProjectBoardPage })),
)
const ProjectBacklogPage = lazy(() =>
  import('@/pages/project-backlog').then((m) => ({ default: m.ProjectBacklogPage })),
)
const ProjectTrashPage = lazy(() =>
  import('@/pages/project-trash').then((m) => ({ default: m.ProjectTrashPage })),
)
const ProjectCustomFieldsPage = lazy(() =>
  import('@/pages/project-custom-fields').then((m) => ({ default: m.ProjectCustomFieldsPage })),
)
const SearchPage = lazy(() => import('@/pages/search'))
const IssueCreatePage = lazy(() =>
  import('@/pages/issue-create').then((m) => ({ default: m.IssueCreatePage })),
)
const IssueDetailPage = lazy(() =>
  import('@/pages/issue-detail').then((m) => ({ default: m.IssueDetailPage })),
)
const LoginPage = lazy(() => import('@/pages/login').then((m) => ({ default: m.LoginPage })))
const SsoCallbackPage = lazy(() =>
  import('@/pages/sso-callback').then((m) => ({ default: m.SsoCallbackPage })),
)
const NotificationsPage = lazy(() =>
  import('@/pages/notifications').then((m) => ({ default: m.NotificationsPage })),
)
const ReportsPage = lazy(() => import('@/pages/reports').then((m) => ({ default: m.ReportsPage })))
const AdminPage = lazy(() => import('@/pages/admin').then((m) => ({ default: m.AdminPage })))

function PageLoader() {
  return (
    <div className="space-y-4 py-3" role="status" aria-label="Загрузка страницы">
      <div className="h-7 w-48 animate-pulse rounded bg-surface-raised" />
      <div className="h-24 animate-pulse rounded-md bg-surface-raised" />
      <div className="h-40 animate-pulse rounded-md bg-surface-raised" />
    </div>
  )
}

const withSuspense = (element: React.ReactElement) => (
  <Suspense fallback={<PageLoader />}>{element}</Suspense>
)

export const router = createBrowserRouter([
  {
    element: <RequireAuth />,
    children: [
      {
        element: <AppShell />,
        children: [
          { path: '/', element: withSuspense(<DashboardPage />) },
          { path: '/projects', element: withSuspense(<ProjectsPage />) },
          { path: '/projects/:projectKey/board', element: withSuspense(<ProjectBoardPage />) },
          { path: '/projects/:projectKey/backlog', element: withSuspense(<ProjectBacklogPage />) },
          { path: '/projects/:projectKey/trash', element: withSuspense(<ProjectTrashPage />) },
          {
            path: '/projects/:projectKey/settings/custom-fields',
            element: withSuspense(<ProjectCustomFieldsPage />),
          },
          { path: '/search', element: withSuspense(<SearchPage />) },
          { path: '/notifications', element: withSuspense(<NotificationsPage />) },
          { path: '/reports', element: withSuspense(<ReportsPage />) },
          { path: '/admin', element: withSuspense(<AdminPage />) },
          { path: '/issues/create', element: withSuspense(<IssueCreatePage />) },
          { path: '/issues/:id', element: withSuspense(<IssueDetailPage />) },
        ],
      },
    ],
  },
  { path: '/login', element: withSuspense(<LoginPage />) },
  { path: '/sso/callback', element: withSuspense(<SsoCallbackPage />) },
  { path: '/register', element: <Navigate to="/login" replace /> },
  { path: '*', element: <Navigate to="/" replace /> },
])
