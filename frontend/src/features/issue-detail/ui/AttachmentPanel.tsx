import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Paperclip, Download, Trash2, FileText, File as FileIcon } from 'lucide-react'
import { useAttachments, useUploadAttachment, useDeleteAttachment } from '@/shared/api/hooks'
import { downloadAttachment } from '@/api/attachment'
import { Button, ConfirmDialog } from '@sdlc/ui/ui'
import { toast } from 'sonner'

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

type UploadProgress = {
  fileName: string
  loaded: number
  total: number
}

export function AttachmentPanel({ issueId }: { issueId: string }) {
  const { t } = useTranslation()
  const { data: attachments = [], isLoading } = useAttachments(issueId)
  const upload = useUploadAttachment(issueId)
  const remove = useDeleteAttachment(issueId)
  const inputRef = useRef<HTMLInputElement>(null)
  const [progress, setProgress] = useState<Record<string, UploadProgress>>({})
  const [uploading, setUploading] = useState(false)
  const [uploadError, setUploadError] = useState(false)
  const [pendingDelete, setPendingDelete] = useState<string | null>(null)

  const onPick = async (files: FileList | null) => {
    if (!files) return
    const selected = Array.from(files)
    if (selected.length === 0) return
    setUploading(true)
    setUploadError(false)
    setProgress(
      Object.fromEntries(
        selected.map((file, index) => [
          index,
          { fileName: file.name, loaded: 0, total: file.size },
        ]),
      ),
    )
    try {
      const results = await Promise.allSettled(
        selected.map((file, index) =>
          upload.mutateAsync({
            file,
            onProgress: (loaded, total) =>
              setProgress((current) => ({
                ...current,
                [index]: { fileName: file.name, loaded, total },
              })),
          }),
        ),
      )
      if (results.every((result) => result.status === 'fulfilled')) {
        toast.success(t('attachments.uploaded', 'Файлы загружены'))
      } else {
        setUploadError(true)
      }
    } catch {
      setUploadError(true)
    } finally {
      setUploading(false)
      setProgress({})
    }
  }

  return (
    <div className="space-y-3" data-testid="attachment-panel">
      <div className="flex items-center justify-between">
        <h3 className="flex items-center gap-2 text-sm font-semibold">
          <Paperclip className="h-4 w-4" aria-hidden />
          {t('attachments.title')}
          {attachments.length > 0 && (
            <span className="text-muted-foreground">({attachments.length})</span>
          )}
        </h3>
        <div>
          <input
            ref={inputRef}
            type="file"
            multiple
            className="hidden"
            data-testid="attachment-input"
            onChange={(e) => {
              void onPick(e.target.files)
              e.target.value = ''
            }}
          />
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => inputRef.current?.click()}
            disabled={uploading}
            aria-label={t('attachments.upload')}
          >
            <Paperclip className="mr-1 h-4 w-4" aria-hidden />
            {uploading ? t('attachments.uploading') : t('attachments.upload')}
          </Button>
        </div>
      </div>

      {uploading &&
        Object.entries(progress).map(([index, item]) => (
          <div key={index} className="space-y-1" data-testid="upload-progress">
            <div className="flex items-center justify-between gap-3 text-xs text-text-muted">
              <span className="truncate">{item.fileName}</span>
              <span>
                {formatSize(item.loaded)} / {formatSize(item.total)}
              </span>
            </div>
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-surface-raised">
              <div
                className="h-full rounded-full bg-accent transition-all"
                style={{
                  width: `${item.total > 0 ? Math.round((item.loaded / item.total) * 100) : 0}%`,
                }}
              />
            </div>
          </div>
        ))}

      {isLoading && <p className="text-sm text-muted-foreground">{t('common.loading')}</p>}

      {!isLoading && attachments.length === 0 && (
        <p className="text-sm text-muted-foreground">{t('attachments.empty')}</p>
      )}

      {attachments.length > 0 && (
        <ul className="divide-y divide-border rounded-md border">
          {attachments.map((a) => (
            <li
              key={a.id}
              className="flex items-center justify-between gap-2 px-3 py-2"
              data-testid="attachment-row"
            >
              <div className="flex min-w-0 items-center gap-2">
                {a.content_type.startsWith('text/') ? (
                  <FileText className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden />
                ) : (
                  <FileIcon className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden />
                )}
                <div className="min-w-0">
                  <p className="truncate text-sm font-medium" title={a.file_name}>
                    {a.file_name}
                  </p>
                  <p className="text-xs text-muted-foreground">{formatSize(a.size_bytes)}</p>
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label={t('attachments.download', { name: a.file_name })}
                  onClick={() => void downloadAttachment(a)}
                >
                  <Download className="h-4 w-4" aria-hidden />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label={t('attachments.delete', { name: a.file_name })}
                  onClick={() => setPendingDelete(a.id)}
                  disabled={remove.isPending}
                >
                  <Trash2 className="h-4 w-4" aria-hidden />
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {uploadError && (
        <p className="text-sm text-destructive" data-testid="attachment-error">
          {t('attachments.uploadFailed')}
        </p>
      )}
      <ConfirmDialog
        open={pendingDelete !== null}
        onOpenChange={(open) => !open && setPendingDelete(null)}
        isPending={remove.isPending}
        error={remove.error?.message}
        title={t('attachments.deleteTitle', 'Удалить файл?')}
        description={t(
          'attachments.deleteConfirm',
          'Файл будет удалён без возможности восстановления.',
        )}
        onConfirm={() =>
          pendingDelete && remove.mutate(pendingDelete, { onSuccess: () => setPendingDelete(null) })
        }
      />
    </div>
  )
}
