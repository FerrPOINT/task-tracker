# Routing — Task Tracker

## 1. Overview

Все frontend-роуты объявлены в `frontend/src/app/router.tsx`. Роуты разделены на public и protected (оборачиваются в `RequireAuth`). Используется `react-router` 8.1.0. Все страницы lazy-loaded с `Suspense`.

## 2. Route Groups

| Group | Auth | Layout |
|-------|------|--------|
| Public | no | — |
| App | yes (`RequireAuth`) | `AppShell` |
| Catch-all | — | redirect to `/` |

## 3. Public Routes

| Route | Page | Notes |
|-------|------|-------|
| `/login` | `pages/login` | Login form |
| `/register` | `pages/register` | Registration form |

## 4. Protected Routes (AppShell)

Все protected-роуты обёрнуты в `RequireAuth` и используют `AppShell` layout.

| Route | Page | Notes |
|-------|------|-------|
| `/` | `pages/dashboard` | Dashboard |
| `/projects` | `pages/projects` | Projects list |
| `/projects/:projectKey/board` | `pages/project-board` | Kanban board |
| `/projects/:projectKey/backlog` | `pages/project-backlog` | Backlog + sprints |
| `/projects/:projectKey/trash` | `pages/project-trash` | Deleted issues |
| `/projects/:projectKey/settings/custom-fields` | `pages/project-custom-fields` | Custom field config |
| `/search` | `pages/search` | Global issue search (JQL) |
| `/notifications` | `pages/notifications` | Notifications |
| `/reports` | `pages/reports` | Reports |
| `/admin` | `pages/admin` | Admin panel |
| `/issues/create` | `pages/issue-create` | Create issue |
| `/issues/:id` | `pages/issue-detail` | Issue detail |

## 5. Catch-all

| Route | Behavior |
|-------|----------|
| `*` | `<Navigate to="/" replace />` — redirect to dashboard |

## 6. Route Configuration (actual router.tsx)

```tsx
// frontend/src/app/router.tsx
import { lazy, Suspense } from 'react'
import { createBrowserRouter, Navigate } from 'react-router'
import { AppShell } from '@/widgets/app-shell'
import { RequireAuth } from '@/shared/auth/require-auth'

// All pages are lazy-loaded for route-level code splitting.
const DashboardPage = lazy(() => import('@/pages/dashboard').then(m => ({ default: m.DashboardPage })))
const ProjectsPage = lazy(() => import('@/pages/projects').then(m => ({ default: m.ProjectsPage })))
const ProjectBoardPage = lazy(() => import('@/pages/project-board').then(m => ({ default: m.ProjectBoardPage })))
const ProjectBacklogPage = lazy(() => import('@/pages/project-backlog').then(m => ({ default: m.ProjectBacklogPage })))
const ProjectTrashPage = lazy(() => import('@/pages/project-trash').then(m => ({ default: m.ProjectTrashPage })))
const ProjectCustomFieldsPage = lazy(() => import('@/pages/project-custom-fields').then(m => ({ default: m.ProjectCustomFieldsPage })))
const SearchPage = lazy(() => import('@/pages/search'))
const IssueCreatePage = lazy(() => import('@/pages/issue-create').then(m => ({ default: m.IssueCreatePage })))
const IssueDetailPage = lazy(() => import('@/pages/issue-detail').then(m => ({ default: m.IssueDetailPage })))
const LoginPage = lazy(() => import('@/pages/login').then(m => ({ default: m.LoginPage })))
const RegisterPage = lazy(() => import('@/pages/register').then(m => ({ default: m.RegisterPage })))
const NotificationsPage = lazy(() => import('@/pages/notifications').then(m => ({ default: m.NotificationsPage })))
const ReportsPage = lazy(() => import('@/pages/reports').then(m => ({ default: m.ReportsPage })))
const AdminPage = lazy(() => import('@/pages/admin').then(m => ({ default: m.AdminPage })))

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
          { path: '/projects/:projectKey/settings/custom-fields', element: withSuspense(<ProjectCustomFieldsPage />) },
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
```

## 7. URL Parameters

| Param | Pattern | Example |
|-------|---------|---------|
| project key | `:projectKey` | `PROJ`, `TT` |
| issue id | `:id` | UUID or issue key |

## 8. Query Parameters

| Param | Used On | Description |
|-------|---------|-------------|
| `jql` | `/search` | JQL filter |
| `page` | list pages | Pagination page |
| `size` | list pages | Page size |
| `sort` | list pages | Sort field,direction |

## 9. Lazy Loading

All page components are lazy-loaded via `React.lazy()` with a `Suspense` wrapper (`PageLoader` fallback). This enables route-level code splitting — each route only downloads the code it needs.

## 10. Not Found Handling

- Unknown routes (`*`) → `<Navigate to="/" replace />` (redirect to dashboard).
- API 404s handled by backend (JSON error response).

## References

- `docs/FRONTEND_ARCHITECTURE.md`
- `docs/UI_UX.md`