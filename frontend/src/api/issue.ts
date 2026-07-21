export interface Issue {
  id: string
  key: string
  summary: string
  description: string
  descriptionHtml?: string
  projectName: string
  projectKey: string
  status: string
  assigneeId: string | null
  assigneeName: string
  assigneeAvatar?: string
  reporterId: string | null
  reporterName: string
  priority: string
  labels: string[]
  dueDate: string | null
  components: string[]
  versions: string[]
  watchers: { id: string; name: string; avatar?: string }[]
  votes: number
  voted: boolean
  originalEstimateSeconds: number | null
  remainingEstimateSeconds: number | null
  timeSpentSeconds: number
}

export const issue: Issue = {
  id: 'issue-1',
  key: 'TT-42',
  summary: 'Implement user authentication',
  description: 'Implement JWT-based authentication for the task tracker.\n\nAcceptance criteria:\n· Login with username/password\n· Access token + httpOnly refresh cookie\n· Password reset via email',
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
  components: ['API'],
  versions: ['v1.0.0'],
  watchers: [
    { id: 'user-1', name: 'Anna' },
    { id: 'user-2', name: 'Ivan' },
  ],
  votes: 2,
  voted: false,
  originalEstimateSeconds: 8 * 3600,
  remainingEstimateSeconds: 4 * 3600,
  timeSpentSeconds: 3 * 3600,
}

export function recalcIssueTime(worklogs: { timeSpentSeconds: number; remainingEstimateSeconds: number | null; startedAt: string }[]) {
  const total = worklogs.reduce((sum, w) => sum + w.timeSpentSeconds, 0)
  issue.timeSpentSeconds = total
  const estimate = issue.originalEstimateSeconds ?? total
  // If the most recent worklog provided a remaining estimate, prefer it; otherwise derive from estimate.
  const sorted = [...worklogs].sort((a, b) => b.startedAt.localeCompare(a.startedAt))
  const latestRemaining = sorted[0]?.remainingEstimateSeconds
  if (latestRemaining !== null && latestRemaining !== undefined) {
    issue.remainingEstimateSeconds = latestRemaining
  } else {
    issue.remainingEstimateSeconds = Math.max(0, estimate - total)
  }
}

export async function getIssue(id: string): Promise<Issue | null> {
  await new Promise((resolve) => setTimeout(resolve, 300))
  if (id !== issue.id) return null
  return { ...issue }
}
