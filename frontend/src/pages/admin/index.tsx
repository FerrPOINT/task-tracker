import { useState } from 'react'
import { format } from 'date-fns'
import { useTranslation } from 'react-i18next'
import {
  useAdminAuditLog,
  useAdminSettings,
  useAdminUsers,
  useCreateAdminUser,
  useUpdateAdminSetting,
  useUpdateAdminUserStatus,
} from '@/shared/api/hooks'
import { Button } from '@sdlc/ui/ui'
import { Card, CardContent, CardHeader, CardTitle } from '@sdlc/ui/ui'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@sdlc/ui/ui'
import { Input } from '@sdlc/ui/ui'
import { Label } from '@sdlc/ui/ui'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@sdlc/ui/ui'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@sdlc/ui/ui'
import { Textarea } from '@sdlc/ui/ui'
import { ErrorState, LoadingState, EmptyState } from '@sdlc/ui/ui'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@sdlc/ui/ui'

type TabValue = 'users' | 'settings' | 'audit'

type CreateUserForm = {
  email: string
  username: string
  display_name: string
  password: string
  is_system_admin: boolean
}

const initialCreateUserForm: CreateUserForm = {
  email: '',
  username: '',
  display_name: '',
  password: '',
  is_system_admin: false,
}

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
  const [tab, setTab] = useState<TabValue>('users')
  const [createOpen, setCreateOpen] = useState(false)
  const [createForm, setCreateForm] = useState<CreateUserForm>(initialCreateUserForm)
  const [statusTarget, setStatusTarget] = useState<{
    id: string
    isActive: boolean
    name: string
  } | null>(null)
  const [settingKey, setSettingKey] = useState('')
  const [settingValue, setSettingValue] = useState('null')
  const [settingError, setSettingError] = useState<string | null>(null)
  const [auditLimit, setAuditLimit] = useState(20)

  const users = useAdminUsers()
  const settings = useAdminSettings()
  const auditLog = useAdminAuditLog(auditLimit)
  const createUser = useCreateAdminUser()
  const updateStatus = useUpdateAdminUserStatus()
  const updateSetting = useUpdateAdminSetting()

  function submitCreateUser(event: React.FormEvent) {
    event.preventDefault()
    createUser.mutate(createForm, {
      onSuccess: () => {
        setCreateOpen(false)
        setCreateForm(initialCreateUserForm)
      },
    })
  }

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
          <TabsTrigger value="users">{t('admin.tabs.users')}</TabsTrigger>
          <TabsTrigger value="settings">{t('admin.tabs.settings')}</TabsTrigger>
          <TabsTrigger value="audit">{t('admin.tabs.audit')}</TabsTrigger>
        </TabsList>

        <TabsContent value="users">
          <Card>
            <CardHeader className="flex-row items-center justify-between gap-3">
              <CardTitle>{t('admin.users.title')}</CardTitle>
              <Button onClick={() => setCreateOpen(true)}>{t('admin.users.create')}</Button>
            </CardHeader>
            <CardContent>
              <QueryState
                isLoading={users.isLoading}
                error={users.error}
                empty={(users.data?.length ?? 0) === 0}
              >
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>{t('admin.users.name')}</TableHead>
                      <TableHead>{t('admin.users.email')}</TableHead>
                      <TableHead>{t('admin.users.role')}</TableHead>
                      <TableHead>{t('admin.users.status')}</TableHead>
                      <TableHead>{t('admin.users.actions')}</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {users.data?.map((user) => (
                      <TableRow key={user.id}>
                        <TableCell>
                          <div className="font-medium">{user.display_name}</div>
                          <div className="text-xs text-text-muted">{user.username}</div>
                        </TableCell>
                        <TableCell>{user.email}</TableCell>
                        <TableCell>
                          {user.is_system_admin
                            ? t('admin.users.systemAdmin')
                            : t('admin.users.member')}
                        </TableCell>
                        <TableCell>
                          {user.is_active ? t('admin.users.active') : t('admin.users.inactive')}
                        </TableCell>
                        <TableCell>
                          <Button
                            variant={user.is_active ? 'outline' : 'secondary'}
                            size="sm"
                            disabled={updateStatus.isPending}
                            onClick={() =>
                              setStatusTarget({
                                id: user.id,
                                isActive: user.is_active,
                                name: user.display_name,
                              })
                            }
                          >
                            {user.is_active
                              ? t('admin.users.deactivate')
                              : t('admin.users.activate')}
                          </Button>
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </QueryState>
            </CardContent>
          </Card>
        </TabsContent>

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

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t('admin.users.create')}</DialogTitle>
          </DialogHeader>
          <form className="space-y-4" onSubmit={submitCreateUser}>
            <div className="space-y-1.5">
              <Label htmlFor="admin-user-email">{t('admin.users.email')}</Label>
              <Input
                id="admin-user-email"
                type="email"
                value={createForm.email}
                onChange={(event) => setCreateForm({ ...createForm, email: event.target.value })}
                required
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="admin-user-username">{t('admin.users.username')}</Label>
              <Input
                id="admin-user-username"
                value={createForm.username}
                onChange={(event) => setCreateForm({ ...createForm, username: event.target.value })}
                required
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="admin-user-display-name">{t('admin.users.displayName')}</Label>
              <Input
                id="admin-user-display-name"
                value={createForm.display_name}
                onChange={(event) =>
                  setCreateForm({ ...createForm, display_name: event.target.value })
                }
                required
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="admin-user-password">{t('admin.users.password')}</Label>
              <Input
                id="admin-user-password"
                type="password"
                value={createForm.password}
                onChange={(event) => setCreateForm({ ...createForm, password: event.target.value })}
                required
              />
            </div>
            <label className="flex items-center gap-2 text-sm text-text-primary">
              <input
                type="checkbox"
                checked={createForm.is_system_admin}
                onChange={(event) =>
                  setCreateForm({ ...createForm, is_system_admin: event.target.checked })
                }
              />
              {t('admin.users.systemAdmin')}
            </label>
            {createUser.error && (
              <p role="alert" className="text-sm text-danger">
                {t('admin.error')}
              </p>
            )}
            <div className="flex justify-end gap-2">
              <Button type="button" variant="secondary" onClick={() => setCreateOpen(false)}>
                {t('common.cancel')}
              </Button>
              <Button type="submit" disabled={createUser.isPending}>
                {t('admin.users.create')}
              </Button>
            </div>
          </form>
        </DialogContent>
      </Dialog>

      <AlertDialog
        open={statusTarget !== null}
        onOpenChange={(open) => !open && setStatusTarget(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {statusTarget?.isActive
                ? t('admin.users.deactivateTitle')
                : t('admin.users.activateTitle')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('admin.users.statusDescription', { name: statusTarget?.name })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="flex justify-end gap-2">
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (!statusTarget) return
                updateStatus.mutate({ id: statusTarget.id, is_active: !statusTarget.isActive })
                setStatusTarget(null)
              }}
            >
              {statusTarget?.isActive ? t('admin.users.deactivate') : t('admin.users.activate')}
            </AlertDialogAction>
          </div>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
