import { createBrowserRouter } from 'react-router'
import { IssueDetailPage } from '@/pages/issue-detail'

export const router = createBrowserRouter([
  {
    path: '/issues/:id',
    element: <IssueDetailPage />,
  },
  {
    path: '/',
    element: <div className="p-8 text-text-muted">Task Tracker home</div>,
  },
])
