import { useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { Paperclip, Download, Trash2, FileText, File as FileIcon } from 'lucide-react'
import {
  useAttachments,
  useUploadAttachment,
  useDeleteAttachment,
} from '@/shared/api/hooks'
import { downloadAttachment } from '@/api/attachment'
import { Button } from '@/shared/ui/button'

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

export function AttachmentPanel({ issueId }: { issueId: string }) {
  const { t } = useTranslation()
  const { data: attachments = [], isLoading } = useAttachments(issueId)
  const upload = useUploadAttachment(issueId)
  const remove = useDeleteAttachment(issueId)
  const inputRef = useRef<HTMLInputElement>(null)

  const onPick = (files: FileList | null) => {
    if (!files) return
    Array.from(files).forEach((f) => upload.mutate(f))
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
              onPick(e.target.files)
              e.target.value = ''
            }}
          />
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => inputRef.current?.click()}
            disabled={upload.isPending}
            aria-label={t('attachments.upload')}
          >
            <Paperclip className="mr-1 h-4 w-4" aria-hidden />
            {upload.isPending ? t('attachments.uploading') : t('attachments.upload')}
          </Button>
        </div>
      </div>

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
                  onClick={() => remove.mutate(a.id)}
                  disabled={remove.isPending}
                >
                  <Trash2 className="h-4 w-4" aria-hidden />
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {upload.isError && (
        <p className="text-sm text-destructive" data-testid="attachment-error">
          {t('attachments.uploadFailed')}
        </p>
      )}
    </div>
  )
}
