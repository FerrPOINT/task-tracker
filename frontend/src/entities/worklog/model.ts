export interface Worklog {
  id: string
  issueId: string
  userId: string
  userDisplayName: string
  timeSpentSeconds: number
  remainingEstimateSeconds: number | null
  startedAt: string
  comment: string | null
  createdAt: string
  updatedAt: string
}

export interface LogWorkInput {
  timeSpent: string
  remainingEstimate?: string
  startedAt?: string
  comment?: string
}

export interface CreateWorklogPayload {
  timeSpentSeconds: number
  remainingEstimateSeconds: number | null
  startedAt: string
  comment: string | null
}
