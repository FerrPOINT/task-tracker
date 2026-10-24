import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Paperclip, Download, Trash2, FileText, File as FileIcon } from 'lucide-react'
import { useAttachments, useUploadAttachment, useDeleteAttachment } from '@/shared/api/hooks'
import { downloadAttachment } from '@/api/attachment'
import {
  Button,
  ConfirmDialog,
  ResourceList,
  ResourceListRow,
  SectionHeader,
  Progress,
  Notice,
  ResourceStateView,
  resourceErrorState,
  type ResourceState,
  toast,
} from '@pdlc/ui/ui'

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
  const { data, isLoading, error, refetch } = useAttachments(issueId)
  const readState: ResourceState = isLoading
    ? { kind: 'loading', message: t('common.loading') }
    : error
      ? resourceErrorState(error, () => void refetch(), {
          hasData: Boolean(data?.length),
          message: t('attachments.loadFailed'),
        })
      : data === undefined
        ? { kind: 'loading', message: t('common.loading') }
        : data.length
          ? { kind: 'ready' }
          : { kind: 'empty', message: t('attachments.empty') }
  const attachments =
    readState.kind === 'ready' || readState.kind === 'partial-error' ? (data ?? []) : []
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
      <SectionHeader
        level={3}
        title={
          <span className="inline-flex items-center gap-2">
            <Paperclip className="h-4 w-4" aria-hidden />
            {t('attachments.title')}
          </span>
        }
        context={attachments.length > 0 ? `(${attachments.length})` : undefined}
        actions={
          <>
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
          </>
        }
      />

      {uploading &&
        Object.entries(progress).map(([index, item]) => (
          <div key={index} className="space-y-1" data-testid="upload-progress">
            <div className="flex items-center justify-between gap-3 text-xs text-text-muted">
              <span className="truncate">{item.fileName}</span>
              <span>
                {formatSize(item.loaded)} / {formatSize(item.total)}
              </span>
            </div>
            <Progress
              value={item.total > 0 ? item.loaded : 0}
              max={item.total > 0 ? item.total : 1}
              aria-label={`${t('attachments.uploading')}: ${item.fileName}`}
              aria-valuetext={`${formatSize(item.loaded)} / ${formatSize(item.total)}`}
            />
          </div>
        ))}

      <ResourceStateView scope="local" state={readState} />

      {attachments.length > 0 && (
        <ResourceList asChild variant="bordered">
          <ul>
            {attachments.map((a) => (
              <ResourceListRow asChild key={a.id}>
                <li
                  className="flex items-center justify-between gap-2"
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
              </ResourceListRow>
            ))}
          </ul>
        </ResourceList>
      )}

      {uploadError && (
        <Notice tone="danger" appearance="inline" data-testid="attachment-error">
          {t('attachments.uploadFailed')}
        </Notice>
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
