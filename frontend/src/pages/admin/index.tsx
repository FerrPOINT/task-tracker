import { useState } from 'react'
import { format } from 'date-fns'
import { useTranslation } from 'react-i18next'
import { useAdminAuditLog, useAdminSettings, useUpdateAdminSetting } from '@/shared/api/hooks'
import { Button } from '@sdlc/ui/ui'
import { Card, CardContent, CardHeader, CardTitle } from '@sdlc/ui/ui'
import { Input } from '@sdlc/ui/ui'
import { Label } from '@sdlc/ui/ui'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@sdlc/ui/ui'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@sdlc/ui/ui'
import { Textarea } from '@sdlc/ui/ui'
import { ErrorState, LoadingState, EmptyState } from '@sdlc/ui/ui'
type TabValue = 'settings' | 'audit'

function formatJson(value: unknown) {
  return JSON.stringify(value, null, 2)
}

function ChangesSummary({ value }: { value: unknown }) {
  const summary = formatJson(value)
  return (
    <span className="line-clamp-2 break-all font-mono text-xs text-text-secondary">{summary}</span>
  )
}

function QueryState({
  isLoading,
  error,
  empty,
  children,
}: {
  isLoading: boolean
  error: unknown
  empty: boolean
  children: React.ReactNode
}) {
  const { t } = useTranslation()
  if (isLoading) return <LoadingState message={t('admin.loading')} />
  if (error) return <ErrorState message={t('admin.error')} />
  if (empty) return <EmptyState message={t('admin.empty')} />
  return <>{children}</>
}

export function AdminPage() {
  const { t } = useTranslation()
  const [tab, setTab] = useState<TabValue>('settings')
  const [settingKey, setSettingKey] = useState('')
  const [settingValue, setSettingValue] = useState('null')
  const [settingError, setSettingError] = useState<string | null>(null)
  const [auditLimit, setAuditLimit] = useState(20)

  const settings = useAdminSettings()
  const auditLog = useAdminAuditLog(auditLimit)
  const updateSetting = useUpdateAdminSetting()

  function submitSetting(event: React.FormEvent) {
    event.preventDefault()
    try {
      const value: unknown = JSON.parse(settingValue)
      setSettingError(null)
      updateSetting.mutate({ key: settingKey.trim(), value })
    } catch {
      setSettingError(t('admin.settings.invalidJson'))
    }
  }

  return (
    <div className="space-y-4">
      <div>
        <h1 className="text-2xl font-bold">{t('admin.title')}</h1>
        <p className="mt-1 text-sm text-text-muted">{t('admin.description')}</p>
      </div>

      <Tabs value={tab} onValueChange={(value) => setTab(value as TabValue)}>
        <TabsList className="h-auto max-w-full flex-wrap justify-start">
          <TabsTrigger value="settings">{t('admin.tabs.settings')}</TabsTrigger>
          <TabsTrigger value="audit">{t('admin.tabs.audit')}</TabsTrigger>
        </TabsList>

        <TabsContent value="settings">
          <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_22rem]">
            <Card>
              <CardHeader>
                <CardTitle>{t('admin.settings.title')}</CardTitle>
              </CardHeader>
              <CardContent>
                <QueryState
                  isLoading={settings.isLoading}
                  error={settings.error}
                  empty={(settings.data?.length ?? 0) === 0}
                >
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>{t('admin.settings.key')}</TableHead>
                        <TableHead>{t('admin.settings.value')}</TableHead>
                        <TableHead>{t('admin.settings.updated')}</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {settings.data?.map((setting) => (
                        <TableRow key={setting.key}>
                          <TableCell className="font-mono text-xs">{setting.key}</TableCell>
                          <TableCell>
                            <ChangesSummary value={setting.value} />
                          </TableCell>
                          <TableCell className="whitespace-nowrap text-xs text-text-muted">
                            {format(new Date(setting.updated_at), 'yyyy-MM-dd HH:mm')}
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </QueryState>
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>{t('admin.settings.update')}</CardTitle>
              </CardHeader>
              <CardContent>
                <form className="space-y-4" onSubmit={submitSetting}>
                  <div className="space-y-1.5">
                    <Label htmlFor="setting-key">{t('admin.settings.key')}</Label>
                    <Input
                      id="setting-key"
                      value={settingKey}
                      onChange={(event) => setSettingKey(event.target.value)}
                      required
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor="setting-value">{t('admin.settings.jsonValue')}</Label>
                    <Textarea
                      id="setting-value"
                      value={settingValue}
                      onChange={(event) => setSettingValue(event.target.value)}
                      className="min-h-36 font-mono"
                      required
                    />
                  </div>
                  {settingError && (
                    <p role="alert" className="text-sm text-danger">
                      {settingError}
                    </p>
                  )}
                  {updateSetting.error && (
                    <p role="alert" className="text-sm text-danger">
                      {t('admin.error')}
                    </p>
                  )}
                  <Button type="submit" disabled={updateSetting.isPending}>
                    {t('admin.settings.save')}
                  </Button>
                </form>
              </CardContent>
            </Card>
          </div>
        </TabsContent>

        <TabsContent value="audit">
          <Card>
            <CardHeader>
              <CardTitle>{t('admin.audit.title')}</CardTitle>
            </CardHeader>
            <CardContent>
              <QueryState
                isLoading={auditLog.isLoading}
                error={auditLog.error}
                empty={(auditLog.data?.length ?? 0) === 0}
              >
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>{t('admin.audit.action')}</TableHead>
                      <TableHead>{t('admin.audit.entity')}</TableHead>
                      <TableHead>{t('admin.audit.actor')}</TableHead>
                      <TableHead>{t('admin.audit.changes')}</TableHead>
                      <TableHead>{t('admin.audit.time')}</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {auditLog.data?.map((entry) => (
                      <TableRow key={entry.id}>
                        <TableCell className="font-mono text-xs">{entry.action}</TableCell>
                        <TableCell>
                          {entry.entity_type}
                          {entry.entity_id ? ` · ${entry.entity_id}` : ''}
                        </TableCell>
                        <TableCell className="font-mono text-xs">
                          {entry.actor_id || t('admin.audit.system')}
                        </TableCell>
                        <TableCell>
                          <ChangesSummary value={entry.metadata} />
                        </TableCell>
                        <TableCell className="whitespace-nowrap text-xs text-text-muted">
                          {format(new Date(entry.created_at), 'yyyy-MM-dd HH:mm')}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
                {(auditLog.data?.length ?? 0) >= auditLimit && (
                  <div className="flex justify-center pt-3">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setAuditLimit((prev) => prev + 20)}
                      disabled={auditLog.isFetching}
                    >
                      {auditLog.isFetching ? t('common.loading') : t('admin.audit.loadMore')}
                    </Button>
                  </div>
                )}
              </QueryState>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  )
}
