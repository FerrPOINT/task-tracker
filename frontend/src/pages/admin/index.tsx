import { useEffect, useRef, useState } from 'react'
import { format } from 'date-fns'
import { Pencil } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import { useAdminAuditLog, useAdminSettings, useUpdateAdminSetting } from '@/shared/api/hooks'
import { ResourceListDescription,
  PageHeader,
  SectionHeader,
  Button,
  ResourceList,
  ResourceListRow,
  Input,
  Label,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Textarea,
  QueryState,
  Notice,
  FormField,
} from '@pdlc/ui/ui'

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

export function AdminPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const tabParam = searchParams.get('tab')
  const tab: TabValue = tabParam === 'audit' ? 'audit' : 'settings'
  const pendingTab = useRef<TabValue | null>(null)
  const [settingKey, setSettingKey] = useState('')
  const [settingValue, setSettingValue] = useState('null')
  const [settingError, setSettingError] = useState<string | null>(null)
  const [saveError, setSaveError] = useState(false)
  const [savedKey, setSavedKey] = useState<string | null>(null)
  const [auditLimit, setAuditLimit] = useState(20)

  const settings = useAdminSettings()
  const auditLog = useAdminAuditLog(auditLimit)
  const updateSetting = useUpdateAdminSetting()

  useEffect(() => {
    if (tabParam === null || tabParam === 'audit') return

    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current)
        next.delete('tab')
        return next
      },
      { replace: true },
    )
  }, [setSearchParams, tabParam])

  useEffect(() => {
    pendingTab.current = null
  }, [tab])

  function changeTab(value: string) {
    const nextTab: TabValue = value === 'audit' ? 'audit' : 'settings'
    if (nextTab === tab || pendingTab.current === nextTab) return

    pendingTab.current = nextTab
    setSearchParams((current) => {
      const next = new URLSearchParams(current)
      if (nextTab === 'audit') next.set('tab', 'audit')
      else next.delete('tab')
      return next
    })
  }

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
      <PageHeader title={t('admin.title')} context={t('admin.description')} />

      <Tabs value={tab} onValueChange={changeTab}>
        <TabsList className="grid h-auto w-full grid-cols-2 gap-1 sm:inline-flex sm:w-auto">
          <TabsTrigger value="settings">{t('admin.tabs.settings')}</TabsTrigger>
          <TabsTrigger value="audit">{t('admin.tabs.audit')}</TabsTrigger>
        </TabsList>

        <TabsContent value="settings">
          <div className="grid gap-4 lg:max-w-6xl lg:grid-cols-[minmax(0,2fr)_minmax(18rem,1fr)]">
            <section className="min-w-0 border-t border-border pt-4">
              <SectionHeader level={2} className="mb-3" title={<>{t('admin.settings.title')}</>} />
              <QueryState
                isLoading={settings.isLoading}
                error={settings.error}
                data={settings.data}
                isEmpty={(items) => items.length === 0}
                errorMessage={t('admin.settings.loadError')}
                onRetry={() => void settings.refetch()}
              >
                <ResourceList asChild variant="bordered">
                  <div role="list">
                    {settings.data?.map((setting) => (
                      <ResourceListRow asChild key={setting.key}>
                        <div
                          role="listitem"

                          className="grid min-w-0 grid-cols-[minmax(0,1fr)_2.75rem] items-center gap-2"
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

                            title={t('admin.settings.edit', { key: setting.key })}
                            aria-label={t('admin.settings.edit', { key: setting.key })}
                            onClick={() => editSetting(setting.key, setting.value)}
                            disabled={updateSetting.isPending}
                          >
                            <Pencil className="h-4 w-4" aria-hidden="true" />
                          </Button>
                        </div>
                      </ResourceListRow>
                    ))}
                  </div>
                </ResourceList>
              </QueryState>
            </section>
            <section className="min-w-0 border-t border-border pt-4">
              <SectionHeader level={2} className="mb-3" title={<>{t('admin.settings.update')}</>} />
              <form
                className="space-y-4"
                onSubmit={submitSetting}
                aria-busy={updateSetting.isPending}
              >
                <div className="space-y-1.5">
                  <Label htmlFor="setting-key">{t('admin.settings.key')}</Label>
                  <Input
                    id="setting-key"

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
                <FormField id="setting-value" label={t('admin.settings.jsonValue')}>
                  {(baseFieldAttrs) => (
                    <Textarea
                      {...baseFieldAttrs}
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
                  )}
                </FormField>
                {settingError && (
                  <Notice role="alert" tone="danger" appearance="inline">
                    {settingError}
                  </Notice>
                )}
                {saveError && (
                  <Notice role="alert" tone="danger" appearance="inline">
                    {t('admin.settings.saveError')}
                  </Notice>
                )}
                {savedKey && (
                  <Notice role="status" tone="success" appearance="inline">
                    {t('admin.settings.saved', { key: savedKey })}
                  </Notice>
                )}
                <Button
                  type="submit"

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
            <SectionHeader level={2} className="mb-3" title={<>{t('admin.audit.title')}</>} />
            <QueryState
              isLoading={auditLog.isLoading}
              error={auditLog.error}
              data={auditLog.data}
              isEmpty={(items) => items.length === 0}
              errorMessage={t('admin.audit.loadError')}
              onRetry={() => void auditLog.refetch()}
            >
              <ResourceList role="list" variant="bordered">
                <div className="hidden gap-3 bg-surface-raised px-3 py-2 text-xs font-semibold text-text-muted lg:grid lg:grid-cols-[minmax(0,1.1fr)_minmax(0,1.5fr)_minmax(0,1fr)_minmax(0,2fr)_8rem]">
                  <span>{t('admin.audit.action')}</span>
                  <span>{t('admin.audit.entity')}</span>
                  <span>{t('admin.audit.actor')}</span>
                  <span>{t('admin.audit.changes')}</span>
                  <span>{t('admin.audit.time')}</span>
                </div>
                {auditLog.data?.map((entry) => (
                  <ResourceListRow asChild key={entry.id}>
                    <div
                      role="listitem"
                      className="grid min-w-0 gap-2 text-sm lg:grid-cols-[minmax(0,1.1fr)_minmax(0,1.5fr)_minmax(0,1fr)_minmax(0,2fr)_8rem] lg:gap-3"
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
                        <ResourceListDescription asChild className="lg:hidden"><span >
                          {t('admin.audit.changes')}:
                        </span></ResourceListDescription>
                        <ChangesSummary value={entry.metadata} />
                      </div>
                      <div className="text-xs text-text-muted">{formatDate(entry.created_at)}</div>
                    </div>
                  </ResourceListRow>
                ))}
              </ResourceList>
              {(auditLog.data?.length ?? 0) >= auditLimit && auditLimit < 1000 && (
                <div className="flex justify-center pt-3">
                  <Button
                    variant="outline"
                    size="sm"

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
