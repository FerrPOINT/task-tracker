export interface Worklog {
  id: string
  issueId: string
  userId: string
  userDisplayName: string
  timeSpentSeconds: number
  startedAt: string
  comment: string | null
  createdAt: string
  updatedAt: string
}

export interface LogWorkInput {
  timeSpent: string
  startedAt?: string
  comment?: string
}
