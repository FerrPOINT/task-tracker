import { createBrowserRouter, Navigate } from 'react-router'
import { IssueDetailPage } from '@/pages/issue-detail'
import { DashboardPage } from '@/pages/dashboard'
import { ProjectsPage } from '@/pages/projects'
import { ProjectBoardPage } from '@/pages/project-board'
import { ProjectBacklogPage } from '@/pages/project-backlog'
import { SearchPage } from '@/pages/search'
import { IssueCreatePage } from '@/pages/issue-create'
import { LoginPage } from '@/pages/login'
import { RegisterPage } from '@/pages/register'
import { AppShell } from '@/widgets/app-shell'

export const router = createBrowserRouter([
  {
    element: <AppShell />,
    children: [
      { path: '/', element: <DashboardPage /> },
      { path: '/projects', element: <ProjectsPage /> },
      { path: '/projects/:id/board', element: <ProjectBoardPage /> },
      { path: '/projects/:id/backlog', element: <ProjectBacklogPage /> },
      { path: '/search', element: <SearchPage /> },
      { path: '/issues/create', element: <IssueCreatePage /> },
      { path: '/issues/:id', element: <IssueDetailPage /> },
    ],
  },
  { path: '/login', element: <LoginPage /> },
  { path: '/register', element: <RegisterPage /> },
  { path: '*', element: <Navigate to="/" replace /> },
])
