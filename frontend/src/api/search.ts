import type { Issue } from './issue'
export type { Issue } from './issue'

export interface SearchFilters {
  projectKey?: string
  status?: string
  assigneeId?: string
  priority?: string
}

export const searchResults: Issue[] = [
  {
    id: 'issue-42',
    key: 'TT-42',
    summary: 'Implement user authentication',
    description: '',
    projectName: 'Task Tracker',
    projectKey: 'TT',
    status: 'In Progress',
    assigneeId: 'user-2',
    assigneeName: 'Ivan',
    reporterId: 'user-1',
    reporterName: 'Anna',
    priority: 'High',
    labels: ['backend', 'auth'],
    dueDate: '2026-08-15',
    components: [],
    versions: [],
    watchers: [],
    votes: 0,
    voted: false,
    originalEstimateSeconds: 8 * 3600,
    remainingEstimateSeconds: 4 * 3600,
    timeSpentSeconds: 3 * 3600,
  },
  {
    id: 'issue-7',
    key: 'TT-7',
    summary: 'OAuth provider integration',
    description: '',
    projectName: 'Task Tracker',
    projectKey: 'TT',
    status: 'To Do',
    assigneeId: 'user-2',
    assigneeName: 'Ivan',
    reporterId: 'user-1',
    reporterName: 'Anna',
    priority: 'High',
    labels: ['backend', 'auth'],
    dueDate: null,
    components: [],
    versions: [],
    watchers: [],
    votes: 0,
    voted: false,
    originalEstimateSeconds: 12 * 3600,
    remainingEstimateSeconds: 8 * 3600,
    timeSpentSeconds: 4 * 3600,
  },
  {
    id: 'issue-2',
    key: 'TT-2',
    summary: 'Setup CI/CD pipeline',
    description: '',
    projectName: 'Task Tracker',
    projectKey: 'TT',
    status: 'To Do',
    assigneeId: 'user-3',
    assigneeName: 'Anna',
    reporterId: 'user-1',
    reporterName: 'Anna',
    priority: 'Medium',
    labels: ['devops'],
    dueDate: null,
    components: [],
    versions: [],
    watchers: [],
    votes: 0,
    voted: false,
    originalEstimateSeconds: 2 * 3600,
    remainingEstimateSeconds: 2 * 3600,
    timeSpentSeconds: 0,
  },
]

export async function searchIssues(_jql: string, _filters?: SearchFilters): Promise<Issue[]> {
  await new Promise((resolve) => setTimeout(resolve, 300))
  return [...searchResults]
}
