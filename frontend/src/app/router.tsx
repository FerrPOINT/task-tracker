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
const RegisterPage = lazy(() =>
  import('@/pages/register').then((m) => ({ default: m.RegisterPage })),
)
const NotificationsPage = lazy(() =>
  import('@/pages/notifications').then((m) => ({ default: m.NotificationsPage })),
)
const ReportsPage = lazy(() => import('@/pages/reports').then((m) => ({ default: m.ReportsPage })))
const AdminPage = lazy(() => import('@/pages/admin').then((m) => ({ default: m.AdminPage })))

function PageLoader() {
  return (
    <div className="flex items-center justify-center py-16 text-sm text-text-muted">Loading…</div>
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
  { path: '/register', element: withSuspense(<RegisterPage />) },
  { path: '*', element: <Navigate to="/" replace /> },
])
