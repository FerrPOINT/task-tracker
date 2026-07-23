import type { Issue } from './issue'
import { currentUser } from './auth'

export type IssueType = 'Task' | 'Story' | 'Bug' | 'Epic'
export type Priority = 'Highest' | 'High' | 'Medium' | 'Low' | 'Lowest'

export interface CreateIssueInput {
  projectId: string
  type: IssueType
  summary: string
  description?: string
  priority?: Priority
  assigneeId?: string
  labels?: string[]
  dueDate?: string | null
}

export async function createIssue(input: CreateIssueInput): Promise<Issue> {
  await new Promise((resolve) => setTimeout(resolve, 300))
  return {
    id: `issue-${Date.now()}`,
    key: 'TT-NEW',
    summary: input.summary,
    description: input.description ?? '',
    projectName: input.projectId === 'project-1' ? 'Task Tracker' : 'Unknown',
    projectKey: input.projectId === 'project-1' ? 'TT' : 'UNK',
    status: 'To Do',
    assigneeId: input.assigneeId ?? null,
    assigneeName: input.assigneeId === 'user-2' ? 'Ivan' : input.assigneeId === 'user-3' ? 'Anna' : input.assigneeId === 'user-4' ? 'Petr' : 'Unassigned',
    reporterId: currentUser.id,
    reporterName: currentUser.displayName,
    priority: input.priority ?? 'Medium',
    labels: input.labels ?? [],
    dueDate: input.dueDate ?? null,
    components: [],
    versions: [],
    watchers: [],
    votes: 0,
    voted: false,
    originalEstimateSeconds: null,
    remainingEstimateSeconds: null,
    timeSpentSeconds: 0,
  }
}
