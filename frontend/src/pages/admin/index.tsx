import { useState } from 'react'
import { format } from 'date-fns'
import { Pencil } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useAdminAuditLog, useAdminSettings, useUpdateAdminSetting } from '@/shared/api/hooks'
import { Button } from '@sdlc/ui/ui'
import { Input } from '@sdlc/ui/ui'
import { Label } from '@sdlc/ui/ui'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@sdlc/ui/ui'
import { Textarea } from '@sdlc/ui/ui'
import { ErrorState, LoadingState, EmptyState } from '@sdlc/ui/ui'
type TabValue = 'settings' | 'audit'

function formatJson(value: unknown) {
  return JSON.stringify(value, null, 2) ?? String(value)
}

function formatDate(value: string) {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : format(date, 'yyyy-MM-dd HH:mm')
}

function ChangesSummary({ value }: { value: unknown }) {
  const summary = formatJson(value)
  return (
    <span className="line-clamp-2 break-all font-mono text-xs text-text-secondary" title={summary}>
      {summary}
    </span>
  )
}

function QueryState({
  isLoading,
  error,
  empty,
  errorMessage,
  onRetry,
  children,
}: {
  isLoading: boolean
  error: unknown
  empty: boolean
  errorMessage: string
  onRetry: () => void
  children: React.ReactNode
}) {
  const { t } = useTranslation()
  if (isLoading) return <LoadingState message={t('admin.loading')} />
  if (error) return <ErrorState message={errorMessage} onRetry={onRetry} />
  if (empty) return <EmptyState message={t('admin.empty')} />
  return <>{children}</>
}

export function AdminPage() {
  const { t } = useTranslation()
  const [tab, setTab] = useState<TabValue>('settings')
  const [settingKey, setSettingKey] = useState('')
  const [settingValue, setSettingValue] = useState('null')
  const [settingError, setSettingError] = useState<string | null>(null)
  const [saveError, setSaveError] = useState(false)
  const [savedKey, setSavedKey] = useState<string | null>(null)
  const [auditLimit, setAuditLimit] = useState(20)

  const settings = useAdminSettings()
  const auditLog = useAdminAuditLog(auditLimit)
  const updateSetting = useUpdateAdminSetting()

  async function submitSetting(event: React.FormEvent) {
    event.preventDefault()
    const key = settingKey.trim()
    if (!key) {
      setSettingError(t('admin.settings.requiredKey'))
      return
    }
    let value: unknown
    try {
      value = JSON.parse(settingValue)
    } catch {
      setSettingError(t('admin.settings.invalidJson'))
      return
    }
    setSettingError(null)
    setSaveError(false)
    setSavedKey(null)
    try {
      await updateSetting.mutateAsync({ key, value })
      setSavedKey(key)
    } catch {
      setSaveError(true)
    }
  }

  function editSetting(key: string, value: unknown) {
    setSettingKey(key)
    setSettingValue(formatJson(value))
    setSettingError(null)
    setSaveError(false)
    setSavedKey(null)
    document.getElementById('setting-value')?.focus()
  }

  return (
    <div className="space-y-4">
      <div>
        <h1 className="text-2xl font-bold">{t('admin.title')}</h1>
        <p className="mt-1 text-sm text-text-muted">{t('admin.description')}</p>
      </div>

      <Tabs value={tab} onValueChange={(value) => setTab(value as TabValue)}>
        <TabsList className="grid h-auto w-full grid-cols-2 gap-1 sm:inline-flex sm:w-auto">
          <TabsTrigger
            value="settings"
            className="min-h-11 px-2 text-xs sm:min-h-8 sm:px-3 sm:text-sm"
          >
            {t('admin.tabs.settings')}
          </TabsTrigger>
          <TabsTrigger
            value="audit"
            className="min-h-11 px-2 text-xs sm:min-h-8 sm:px-3 sm:text-sm"
          >
            {t('admin.tabs.audit')}
          </TabsTrigger>
        </TabsList>

        <TabsContent value="settings">
          <div className="grid gap-4 lg:max-w-6xl lg:grid-cols-[minmax(0,2fr)_minmax(18rem,1fr)]">
            <section className="min-w-0 border-t border-border pt-4">
              <h2 className="mb-3 text-sm font-semibold">{t('admin.settings.title')}</h2>
              <QueryState
                isLoading={settings.isLoading}
                error={settings.error}
                empty={(settings.data?.length ?? 0) === 0}
                errorMessage={t('admin.settings.loadError')}
                onRetry={() => void settings.refetch()}
              >
                <div role="list" className="divide-y divide-border rounded-md border border-border">
                  {settings.data?.map((setting) => (
                    <div
                      role="listitem"
                      key={setting.key}
                      className="grid min-w-0 grid-cols-[minmax(0,1fr)_2.75rem] items-center gap-2 p-3"
                    >
                      <div className="min-w-0 space-y-1">
                        <div className="break-all font-mono text-xs font-semibold">
                          {setting.key}
                        </div>
                        <ChangesSummary value={setting.value} />
                        <div className="text-xs text-text-muted">
                          {t('admin.settings.updated')}: {formatDate(setting.updated_at)}
                        </div>
                      </div>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        className="h-11 w-11"
                        title={t('admin.settings.edit', { key: setting.key })}
                        aria-label={t('admin.settings.edit', { key: setting.key })}
                        onClick={() => editSetting(setting.key, setting.value)}
                        disabled={updateSetting.isPending}
                      >
                        <Pencil className="h-4 w-4" aria-hidden="true" />
                      </Button>
                    </div>
                  ))}
                </div>
              </QueryState>
            </section>
            <section className="min-w-0 border-t border-border pt-4">
              <h2 className="mb-3 text-sm font-semibold">{t('admin.settings.update')}</h2>
              <form
                className="space-y-4"
                onSubmit={submitSetting}
                aria-busy={updateSetting.isPending}
              >
                <div className="space-y-1.5">
                  <Label htmlFor="setting-key">{t('admin.settings.key')}</Label>
                  <Input
                    id="setting-key"
                    className="min-h-11 sm:min-h-9"
                    value={settingKey}
                    onChange={(event) => {
                      setSettingKey(event.target.value)
                      setSettingError(null)
                      setSaveError(false)
                      setSavedKey(null)
                    }}
                    disabled={updateSetting.isPending}
                    required
                    list="known-setting-keys"
                  />
                  <datalist id="known-setting-keys">
                    {settings.data?.map((setting) => (
                      <option key={setting.key} value={setting.key} />
                    ))}
                  </datalist>
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="setting-value">{t('admin.settings.jsonValue')}</Label>
                  <Textarea
                    id="setting-value"
                    value={settingValue}
                    onChange={(event) => {
                      setSettingValue(event.target.value)
                      setSettingError(null)
                      setSaveError(false)
                      setSavedKey(null)
                    }}
                    className="min-h-36 font-mono"
                    disabled={updateSetting.isPending}
                    required
                  />
                </div>
                {settingError && (
                  <p role="alert" className="text-sm text-danger">
                    {settingError}
                  </p>
                )}
                {saveError && (
                  <p role="alert" className="text-sm text-danger">
                    {t('admin.settings.saveError')}
                  </p>
                )}
                {savedKey && (
                  <p role="status" className="text-sm text-success">
                    {t('admin.settings.saved', { key: savedKey })}
                  </p>
                )}
                <Button
                  type="submit"
                  className="min-h-11 sm:min-h-9"
                  disabled={updateSetting.isPending}
                >
                  {updateSetting.isPending ? t('common.loading') : t('admin.settings.save')}
                </Button>
              </form>
            </section>
          </div>
        </TabsContent>

        <TabsContent value="audit">
          <section className="min-w-0 border-t border-border pt-4">
            <h2 className="mb-3 text-sm font-semibold">{t('admin.audit.title')}</h2>
            <QueryState
              isLoading={auditLog.isLoading}
              error={auditLog.error}
              empty={(auditLog.data?.length ?? 0) === 0}
              errorMessage={t('admin.audit.loadError')}
              onRetry={() => void auditLog.refetch()}
            >
              <div role="list" className="divide-y divide-border rounded-md border border-border">
                <div className="hidden gap-3 bg-surface-raised px-3 py-2 text-xs font-semibold text-text-muted lg:grid lg:grid-cols-[minmax(0,1.1fr)_minmax(0,1.5fr)_minmax(0,1fr)_minmax(0,2fr)_8rem]">
                  <span>{t('admin.audit.action')}</span>
                  <span>{t('admin.audit.entity')}</span>
                  <span>{t('admin.audit.actor')}</span>
                  <span>{t('admin.audit.changes')}</span>
                  <span>{t('admin.audit.time')}</span>
                </div>
                {auditLog.data?.map((entry) => (
                  <div
                    role="listitem"
                    key={entry.id}
                    className="grid min-w-0 gap-2 p-3 text-sm lg:grid-cols-[minmax(0,1.1fr)_minmax(0,1.5fr)_minmax(0,1fr)_minmax(0,2fr)_8rem] lg:gap-3"
                  >
                    <div className="min-w-0 break-all font-mono text-xs font-semibold">
                      {entry.action}
                    </div>
                    <div className="min-w-0 break-all text-xs">
                      <span className="mr-1 text-text-muted lg:hidden">
                        {t('admin.audit.entity')}:
                      </span>
                      {entry.entity_type}
                      {entry.entity_id ? ` · ${entry.entity_id}` : ''}
                    </div>
                    <div className="min-w-0 break-all font-mono text-xs">
                      <span className="mr-1 font-sans text-text-muted lg:hidden">
                        {t('admin.audit.actor')}:
                      </span>
                      {entry.actor_id || t('admin.audit.system')}
                    </div>
                    <div className="min-w-0">
                      <span className="text-xs text-text-muted lg:hidden">
                        {t('admin.audit.changes')}:
                      </span>
                      <ChangesSummary value={entry.metadata} />
                    </div>
                    <div className="text-xs text-text-muted">{formatDate(entry.created_at)}</div>
                  </div>
                ))}
              </div>
              {(auditLog.data?.length ?? 0) >= auditLimit && auditLimit < 1000 && (
                <div className="flex justify-center pt-3">
                  <Button
                    variant="outline"
                    size="sm"
                    className="min-h-11 sm:min-h-9"
                    onClick={() => setAuditLimit((prev) => Math.min(prev + 20, 1000))}
                    disabled={auditLog.isFetching}
                  >
                    {auditLog.isFetching ? t('common.loading') : t('admin.audit.loadMore')}
                  </Button>
                </div>
              )}
            </QueryState>
          </section>
        </TabsContent>
      </Tabs>
    </div>
  )
}
