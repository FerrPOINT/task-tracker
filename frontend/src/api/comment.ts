import type { Comment, CreateCommentInput, UpdateCommentInput } from '@/entities/comment/model'
import { api } from './client'

function mapDto(c: {
  id: string
  issue_id: string
  author_id: string
  author_name?: string | null
  body: string
  created_at: string
  updated_at: string
}): Comment {
  return {
    id: c.id,
    issueId: c.issue_id,
    authorId: c.author_id,
    authorName: c.author_name,
    body: c.body,
    createdAt: c.created_at,
    updatedAt: c.updated_at,
  }
}

export async function listComments(issueId: string): Promise<Comment[]> {
  const { data, error } = await api.GET('/api/v1/issues/{issue_id}/comments', {
    params: { path: { issue_id: issueId } },
  })
  if (error || !data) throw new Error('Failed to load comments')
  return data.comments.map(mapDto).sort((a, b) => b.createdAt.localeCompare(a.createdAt))
}

export async function createComment(
  issueId: string,
  input: CreateCommentInput,
): Promise<Comment> {
  const { data, error } = await api.POST('/api/v1/issues/{issue_id}/comments', {
    params: { path: { issue_id: issueId } },
    body: { body: input.body.trim() },
  })
  if (error || !data) throw new Error('Failed to create comment')
  return mapDto(data)
}

export async function updateComment(
  commentId: string,
  input: UpdateCommentInput,
): Promise<Comment> {
  const { data, error } = await api.PATCH('/api/v1/comments/{id}', {
    params: { path: { id: commentId } },
    body: { body: input.body.trim() },
  })
  if (error || !data) throw new Error('Failed to update comment')
  return mapDto(data)
}

export async function deleteComment(commentId: string): Promise<void> {
  const { error } = await api.DELETE('/api/v1/comments/{id}', {
    params: { path: { id: commentId } },
  })
  if (error) throw new Error('Failed to delete comment')
}
