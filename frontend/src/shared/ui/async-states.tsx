import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Loader2, RotateCcw } from 'lucide-react'
import { Button } from './button'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogHeader,
  AlertDialogTitle,
} from './alert-dialog'

interface LoadingStateProps {
  message?: string
}

export function LoadingState({ message }: LoadingStateProps) {
  const { t } = useTranslation()
  return (
    <div className="flex items-center justify-center gap-2 p-4 text-sm text-text-muted">
      <Loader2 className="h-4 w-4 animate-spin" />
      {message ?? t('common.loading')}
    </div>
  )
}

interface EmptyStateProps {
  message: string
  action?: ReactNode
}

export function EmptyState({ message, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 p-8 text-center text-sm text-text-muted">
      <p>{message}</p>
      {action}
    </div>
  )
}

interface ErrorStateProps {
  message: string
  onRetry?: () => void
}

export function ErrorState({ message, onRetry }: ErrorStateProps) {
  const { t } = useTranslation()
  return (
    <div className="flex flex-col items-center justify-center gap-3 p-4 text-center">
      <p role="alert" className="text-sm text-danger">
        {message}
      </p>
      {onRetry && (
        <Button variant="outline" size="sm" onClick={onRetry} className="gap-1">
          <RotateCcw className="h-3.5 w-3.5" />
          {t('common.retry')}
        </Button>
      )}
    </div>
  )
}

interface ConfirmDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description: string
  onConfirm: () => void
}

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  onConfirm,
}: ConfirmDialogProps) {
  const { t } = useTranslation()
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
        </AlertDialogHeader>
        <div className="flex justify-end gap-2 pt-2">
          <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
          <AlertDialogAction onClick={onConfirm}>{t('common.confirm')}</AlertDialogAction>
        </div>
      </AlertDialogContent>
    </AlertDialog>
  )
}
