# Routing — Task Tracker

## 1. Overview

Все frontend-роуты объявлены в `frontend/src/app/router.tsx`. Роуты разделены на public и protected. Используется `react-router` 8.1.0.

## 2. Route Groups

| Group | Prefix | Auth |
|-------|--------|------|
| Public | `/login`, `/register`, `/forgot-password`, `/reset-password` | no |
| App | `/` и далее | yes |
| Admin | `/admin/*` | system admin |
| API | `/api/v1/*` | по endpoint |
| WebSocket | `/ws/v1/*` | yes |

## 3. Public Routes

| Route | Page | Notes |
|-------|------|-------|
| `/login` | `pages/login` | Redirect to `/` if already authenticated |
| `/register` | `pages/register` | If registration enabled |
| `/forgot-password` | `pages/forgot-password` | Email form |
| `/reset-password` | `pages/reset-password` | Token + new password |
| `/invite/accept` | `pages/invite-accept` | Accept project invitation |

## 4. App Shell Routes

| Route | Page | Layout |
|-------|------|--------|
| `/` | `pages/dashboard` | AppLayout |
| `/notifications` | `pages/notifications` | AppLayout |
| `/settings` | `pages/settings` | AppLayout |
| `/settings/profile` | `pages/settings/profile` | AppLayout |
| `/settings/account` | `pages/settings/account` | AppLayout |
| `/settings/notifications` | `pages/settings/notifications` | AppLayout |
| `/settings/locale` | `pages/settings/locale` | AppLayout |

## 5. Projects

| Route | Page | Notes |
|-------|------|-------|
| `/projects` | `pages/project-list` | All projects |
| `/projects/create` | `pages/project-create` | Requires `create_project` permission |
| `/projects/:key` | `pages/project` | Project overview |
| `/projects/:key/issues` | `pages/project/issues` | Project issue list |
| `/projects/:key/backlog` | `pages/project/backlog` | Backlog + sprints |
| `/projects/:key/boards` | `pages/project/boards` | Boards list |
| `/projects/:key/boards/:boardId` | `pages/board` | Kanban/scrum board |
| `/projects/:key/reports` | `pages/reports` | Reports hub |
| `/projects/:key/reports/velocity` | `pages/reports/velocity` | Velocity chart |
| `/projects/:key/reports/burndown` | `pages/reports/burndown` | Sprint burndown |
| `/projects/:key/reports/cumulative-flow` | `pages/reports/cumulative-flow` | CFD |
| `/projects/:key/versions` | `pages/project/versions` | Releases |
| `/projects/:key/components` | `pages/project/components` | Components |
| `/projects/:key/settings` | `pages/project/settings` | Project settings |
| `/projects/:key/settings/roles` | `pages/project/settings/roles` | Roles |
| `/projects/:key/settings/permissions` | `pages/project/settings/permissions` | Permissions |
| `/projects/:key/settings/issue-types` | `pages/project/settings/issue-types` | Issue type scheme |
| `/projects/:key/settings/workflows` | `pages/project/settings/workflows` | Workflow scheme |
| `/projects/:key/settings/screens` | `pages/project/settings/screens` | Screen scheme |
| `/projects/:key/settings/boards` | `pages/project/settings/boards` | Board config |
| `/projects/:key/settings/notifications` | `pages/project/settings/notifications` | Notification scheme |

## 6. Issues

| Route | Page | Notes |
|-------|------|-------|
| `/issues` | `pages/search` | Global issue navigator (JQL) |
| `/issues/:key` | `pages/issue` | Issue detail |
| `/issues/:key/edit` | `pages/issue/edit` | Edit issue |
| `/issues/:key/transition/:transitionId` | action | Transition dialog |
| `/create-issue` | `pages/create-issue` | Global create issue |
| `/projects/:key/create-issue` | `pages/create-issue` | Create issue in project |

## 7. Filters and Dashboards

| Route | Page | Notes |
|-------|------|-------|
| `/filters` | `pages/filters` | Saved filters |
| `/filters/:filterId` | `pages/search` | Load saved filter |
| `/dashboards` | `pages/dashboards` | Dashboard list |
| `/dashboards/:dashboardId` | `pages/dashboard` | Dashboard |

## 8. Admin

| Route | Page | Notes |
|-------|------|-------|
| `/admin` | `pages/admin` | Admin dashboard |
| `/admin/users` | `pages/admin/users` | User management |
| `/admin/groups` | `pages/admin/groups` | Groups |
| `/admin/projects` | `pages/admin/projects` | All projects |
| `/admin/settings` | `pages/admin/settings` | Instance settings |
| `/admin/audit-log` | `pages/admin/audit-log` | Audit log |
| `/admin/backup` | `pages/admin/backup` | Backup/restore |
| `/admin/license` | `pages/admin/license` | License (enterprise) |

## 9. Catch-all

| Route | Page | Notes |
|-------|------|-------|
| `*` | `pages/not-found` | 404 |

## 10. Route Configuration Example

```tsx
// frontend/src/app/router.tsx
import { createBrowserRouter, Navigate } from "react-router"
import { AppLayout } from "@/app/layouts/AppLayout"
import { AuthLayout } from "@/app/layouts/AuthLayout"
import { ProtectedRoute } from "@/app/routes/ProtectedRoute"
import { AdminRoute } from "@/app/routes/AdminRoute"

export const router = createBrowserRouter([
  {
    element: <AuthLayout />,
    children: [
      { path: "/login", element: <LoginPage /> },
      { path: "/register", element: <RegisterPage /> },
      { path: "/forgot-password", element: <ForgotPasswordPage /> },
      { path: "/reset-password", element: <ResetPasswordPage /> },
    ],
  },
  {
    element: <ProtectedRoute />,
    children: [
      {
        element: <AppLayout />,
        children: [
          { path: "/", element: <DashboardPage /> },
          { path: "/notifications", element: <NotificationsPage /> },
          { path: "/settings/*", element: <SettingsPage /> },
          { path: "/projects", element: <ProjectListPage /> },
          { path: "/projects/create", element: <ProjectCreatePage /> },
          { path: "/projects/:key/*", element: <ProjectShellPage /> },
          { path: "/issues", element: <SearchPage /> },
          { path: "/issues/:key", element: <IssuePage /> },
          { path: "/filters/*", element: <FiltersPage /> },
          { path: "/dashboards/*", element: <DashboardsPage /> },
        ],
      },
      {
        element: <AdminRoute />,
        children: [
          { path: "/admin/*", element: <AdminPage /> },
        ],
      },
    ],
  },
  { path: "*", element: <NotFoundPage /> },
])
```

## 11. Project Shell Nested Routes

```tsx
// pages/project/routes.tsx
export const projectRoutes = [
  { index: true, element: <ProjectOverviewPage /> },
  { path: "issues", element: <ProjectIssuesPage /> },
  { path: "backlog", element: <BacklogPage /> },
  { path: "boards", element: <BoardsListPage /> },
  { path: "boards/:boardId", element: <BoardPage /> },
  { path: "reports/*", element: <ReportsPage /> },
  { path: "versions", element: <VersionsPage /> },
  { path: "components", element: <ComponentsPage /> },
  { path: "settings/*", element: <ProjectSettingsPage /> },
]
```

## 12. URL Parameters

| Param | Pattern | Example |
|-------|---------|---------|
| project key | `[A-Z]{2,10}` | `PROJ`, `TT` |
| issue key | `{projectKey}-{number}` | `PROJ-123` |
| board id | UUID | `550e8400-e29b-41d4-a716-446655440000` |
| sprint id | UUID | `...` |

## 13. Query Parameters

| Param | Used On | Description |
|-------|---------|-------------|
| `jql` | `/issues`, `/projects/:key/issues` | JQL filter |
| `page` | list pages | Pagination page |
| `size` | list pages | Page size |
| `sort` | list pages | Sort field,direction |
| `view` | `/projects/:key/boards/:boardId` | board view (board/backlog) |
| `modal` | any | open modal (create-issue, etc.) |

## 14. Redirects

| From | To | Condition |
|------|-----|-----------|
| `/` after login | `/` dashboard | — |
| `/login` | `/` | already authenticated |
| `/projects/:key` | `/projects/:key/issues` | default project tab |
| `/projects/:key/boards` | `/projects/:key/boards/{defaultBoardId}` | if single board |

## 15. Modals as Routes

Для прямой ссылки и совместимости с refresh:

| Modal | Route |
|-------|-------|
| Create issue | `/projects/:key/create-issue` |
| Issue transition | `/issues/:key/transition/:transitionId` |
| User picker | `?modal=user-picker` |

## 16. Breadcrumbs

Breadcrumbs формируются автоматически по роуту:

```
Projects / PROJ / Issues / PROJ-123
```

## 17. API and WebSocket Routes

| Route | Description |
|-------|-------------|
| `/api/v1/*` | REST API |
| `/ws/v1/connect` | WebSocket |
| `/health` | Liveness |
| `/health/ready` | Readiness |
| `/metrics` | Prometheus |

## 18. Permissions per Route

| Route | Required Permission |
|-------|---------------------|
| `/projects/create` | `create_project` |
| `/projects/:key/settings/*` | project admin |
| `/admin/*` | `system_admin` |
| `/projects/:key/backlog` | `browse_projects` + scrum board |
| `/projects/:key/reports/*` | `browse_projects` |

## 19. Lazy Loading

```tsx
const ProjectPage = lazy(() => import("@/pages/project"))
const AdminPage = lazy(() => import("@/pages/admin"))
```

## 20. Not Found Handling

- Unknown app routes → `NotFoundPage`.
- Unknown API routes → 404 JSON.
- Project not found → 404 page с сообщением.
- Issue not found → 404 page.
