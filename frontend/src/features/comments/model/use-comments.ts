import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  listComments,
  createComment,
  updateComment,
  deleteComment,
} from '@/api/comment'
import type { Comment, CreateCommentInput, UpdateCommentInput } from '@/entities/comment/model'

export function useComments(issueId: string) {
  return useQuery<Comment[]>({
    queryKey: ['comments', issueId],
    queryFn: () => listComments(issueId),
    enabled: Boolean(issueId),
  })
}

export function useCreateComment(issueId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: CreateCommentInput) => createComment(issueId, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['comments', issueId] })
    },
  })
}

export function useUpdateComment(issueId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdateCommentInput }) =>
      updateComment(id, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['comments', issueId] })
    },
  })
}

export function useDeleteComment(issueId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => deleteComment(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['comments', issueId] })
    },
  })
}
