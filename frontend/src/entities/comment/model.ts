export interface Comment {
  id: string
  issueId: string
  authorId: string
  authorName?: string | null
  body: string
  createdAt: string
  updatedAt: string
}

export interface CreateCommentInput {
  body: string
}

export interface UpdateCommentInput {
  body: string
}
