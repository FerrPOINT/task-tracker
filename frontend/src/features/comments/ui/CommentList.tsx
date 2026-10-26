import { memo, useId, useState } from 'react'
import { CommentBody } from '@/entities/comment/ui/CommentBody'
import { useTranslation } from 'react-i18next'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import {
  ConfirmDialog,
  Button,
  ErrorState,
  Textarea,
  FormField,
  Notice,
  Card,
  CardContent,
  UserAvatar,
} from '@pdlc/ui/ui'

import {
  useComments,
  useCreateComment,
  useUpdateComment,
  useDeleteComment,
} from '../model/use-comments'
import type { Comment, CreateCommentInput, UpdateCommentInput } from '@/entities/comment/model'

interface CommentFormProps {
  onSubmit: (input: CreateCommentInput | UpdateCommentInput) => void | Promise<unknown>
  onCancel?: () => void
  initialBody?: string
  submitLabel?: string
}

export function CommentForm({
  onSubmit,
  onCancel,
  initialBody = '',
  submitLabel,
}: CommentFormProps) {
  const { t } = useTranslation()
  const fieldId = useId()
  const schema = z.object({
    body: z.string().min(1, t('comments.validation.required')),
  })

  const form = useForm<{ body: string }>({
    resolver: zodResolver(schema),
    defaultValues: { body: initialBody },
  })
  const [submitError, setSubmitError] = useState<string | null>(null)

  const handleSubmit = form.handleSubmit(async (values) => {
    setSubmitError(null)
    try {
      await onSubmit({ body: values.body.trim() })
      if (!onCancel) form.reset({ body: '' })
    } catch (error) {
      setSubmitError(error instanceof Error ? error.message : t('common.error'))
    }
  })

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      <FormField
        id={fieldId}
        label={t('comments.body')}
        error={form.formState.errors.body?.message}
      >
        {(attrs) => (
          <Textarea
            {...attrs}
            {...form.register('body')}
            placeholder={t('comments.placeholder')}
            rows={4}
          />
        )}
      </FormField>
      {submitError && (
        <Notice role="alert" tone="danger" appearance="inline">
          {submitError}
        </Notice>
      )}
      <div className="flex justify-end gap-2">
        {onCancel && (
          <Button type="button" variant="secondary" onClick={onCancel}>
            {t('common.cancel')}
          </Button>
        )}
        <Button type="submit" disabled={form.formState.isSubmitting}>
          {submitLabel ?? t('common.save')}
        </Button>
      </div>
    </form>
  )
}

interface CommentItemProps {
  comment: Comment
  currentUserId?: string
  onEdit: (comment: Comment) => void
  onDelete: (commentId: string) => void
}

export const CommentItem = memo(function CommentItem({
  comment,
  currentUserId,
  onEdit,
  onDelete,
}: CommentItemProps) {
  const { t } = useTranslation()
  const isAuthor = currentUserId ? comment.authorId === currentUserId : false
  const date = new Date(comment.createdAt).toLocaleString()

  return (
    <Card>
      <CardContent>
        <div className="mb-2 flex min-w-0 flex-wrap items-center justify-between gap-2">
          <div className="flex min-w-0 items-center gap-2">
            <UserAvatar name={comment.authorName} userId={comment.authorId} size="md" />
            <div className="min-w-0 [overflow-wrap:anywhere]">
              <p className="text-sm font-medium text-text-primary">
                {comment.authorName ?? t('comments.unknown')}
              </p>
              <p className="text-xs text-text-muted">{date}</p>
            </div>
          </div>
          {isAuthor && (
            <div className="flex gap-2">
              <Button variant="ghost" size="sm" onClick={() => onEdit(comment)}>
                {t('common.edit')}
              </Button>
              <Button variant="destructive-outline" size="sm" onClick={() => onDelete(comment.id)}>
                {t('common.delete')}
              </Button>
            </div>
          )}
        </div>
        <CommentBody body={comment.body} />
      </CardContent>
    </Card>
  )
})

interface CommentListProps {
  comments: Comment[]
  currentUserId?: string
  onEdit: (comment: Comment) => void
  onDelete: (commentId: string) => void
}

export function CommentList({ comments, currentUserId, onEdit, onDelete }: CommentListProps) {
  const { t } = useTranslation()
  if (comments.length === 0) {
    return <p className="text-sm text-text-muted">{t('comments.empty')}</p>
  }
  return (
    <div className="space-y-4">
      {comments.map((comment) => (
        <CommentItem
          key={comment.id}
          comment={comment}
          currentUserId={currentUserId}
          onEdit={onEdit}
          onDelete={onDelete}
        />
      ))}
    </div>
  )
}

interface CommentsPanelProps {
  issueId: string
  currentUserId?: string
}

export function CommentsPanel({ issueId, currentUserId }: CommentsPanelProps) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState<Comment | null>(null)
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false)
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null)
  const [deleteError, setDeleteError] = useState<string | null>(null)
  const commentsQuery = useComments(issueId)
  const { data: comments, isLoading } = commentsQuery
  const create = useCreateComment(issueId)
  const update = useUpdateComment(issueId)
  const remove = useDeleteComment(issueId)

  if (isLoading) {
    return <p className="text-sm text-text-muted">{t('common.loading')}</p>
  }

  if (commentsQuery.error && !comments) {
    return (
      <ErrorState message={t('comments.loadError')} onRetry={() => void commentsQuery.refetch()} />
    )
  }

  const handleCreate = (input: CreateCommentInput) => create.mutateAsync(input)

  const handleUpdate = async (input: UpdateCommentInput) => {
    if (!editing) return
    await update.mutateAsync({ id: editing.id, input })
    setEditing(null)
  }

  const handleEdit = (comment: Comment) => {
    setEditing(comment)
  }

  const handleDelete = (commentId: string) => {
    setPendingDeleteId(commentId)
    setDeleteError(null)
    setDeleteConfirmOpen(true)
  }

  return (
    <div className="space-y-6">
      {commentsQuery.error && (
        <ErrorState
          message={t('comments.loadError')}
          onRetry={() => void commentsQuery.refetch()}
        />
      )}
      {editing ? (
        <CommentForm
          initialBody={editing.body}
          onSubmit={handleUpdate}
          onCancel={() => setEditing(null)}
          submitLabel={t('common.save')}
        />
      ) : (
        <CommentForm onSubmit={handleCreate} submitLabel={t('comments.add')} />
      )}
      <CommentList
        comments={comments ?? []}
        currentUserId={currentUserId}
        onEdit={handleEdit}
        onDelete={handleDelete}
      />
      <ConfirmDialog
        open={deleteConfirmOpen}
        onOpenChange={(open) => !remove.isPending && setDeleteConfirmOpen(open)}
        title={t('common.delete')}
        description={t('comments.deleteConfirm')}
        isPending={remove.isPending}
        error={deleteError}
        confirmLabel={t('common.confirm')}
        cancelLabel={t('common.cancel')}
        pendingLabel={t('common.loading')}
        onConfirm={() => {
          if (!pendingDeleteId) return
          remove.mutate(pendingDeleteId, {
            onSuccess: () => setDeleteConfirmOpen(false),
            onError: (error) => setDeleteError(error.message),
          })
        }}
      />
    </div>
  )
}
