import * as React from 'react'
import { useTranslation } from 'react-i18next'
import { X, UserPlus } from 'lucide-react'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from '@/shared/ui/dialog'
import { Button } from '@/shared/ui/button'
import {
  useProjectMembers,
  useUsers,
  useAddProjectMember,
  useRemoveProjectMember,
} from '@/shared/api/hooks'
import type { DirectoryUser } from '@/api/auth'
import { UserAvatar } from '@/shared/ui/user-avatar'

type User = DirectoryUser

export function ProjectMembersPanel({
  projectKey,
  trigger,
}: {
  projectKey: string
  trigger?: React.ReactNode
}) {
  const { t } = useTranslation()
  const { data, isLoading } = useProjectMembers(projectKey)
  const { data: users } = useUsers()
  const add = useAddProjectMember(projectKey)
  const remove = useRemoveProjectMember(projectKey)
  const [open, setOpen] = React.useState(false)
  const [selectedUserId, setSelectedUserId] = React.useState('')

  const memberUserIds = new Set(data?.members.map((m) => m.user_id) ?? [])
  const candidates = users?.filter((u) => !memberUserIds.has(u.id)) ?? []

  function handleAdd(e: React.FormEvent) {
    e.preventDefault()
    if (!selectedUserId) return
    add.mutate({ user_id: selectedUserId, role: 'member' })
    setSelectedUserId('')
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        {trigger ?? (
          <Button variant="outline" size="sm" className="gap-1">
            <UserPlus className="h-4 w-4" />
            <span className="hidden sm:inline">{t('board.members')}</span>
          </Button>
        )}
      </DialogTrigger>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{t('projectMembers.title')}</DialogTitle>
        </DialogHeader>

        {isLoading ? (
          <div className="py-4 text-text-muted">{t('issue.loading')}</div>
        ) : (
          <div className="space-y-4">
            <form onSubmit={handleAdd} className="flex items-end gap-2">
              <div className="flex-1">
                <label className="mb-1 block text-xs text-text-secondary">
                  {t('projectMembers.addUser')}
                </label>
                <select
                  value={selectedUserId}
                  onChange={(e) => setSelectedUserId(e.target.value)}
                  className="w-full rounded-md border border-border bg-surface px-3 py-2 text-sm text-text-primary focus:border-accent focus:outline-none"
                >
                  <option value="">{t('projectMembers.selectUser')}</option>
                  {candidates.map((u: User) => (
                    <option key={u.id} value={u.id}>
                      {u.display_name || u.username}
                    </option>
                  ))}
                </select>
              </div>
              <Button type="submit" size="sm" disabled={!selectedUserId || add.isPending}>
                <UserPlus className="h-4 w-4" />
              </Button>
            </form>

            <div className="max-h-72 space-y-2 overflow-y-auto">
              {data?.members.length === 0 && (
                <div className="py-4 text-center text-sm text-text-muted">
                  {t('projectMembers.empty')}
                </div>
              )}
              {data?.members.map((m) => {
                const user = users?.find((u: User) => u.id === m.user_id)
                const name = user?.display_name || user?.username || m.user_id
                return (
                  <div
                    key={m.user_id}
                    className="flex items-center justify-between rounded-md border border-border bg-surface-raised p-2"
                  >
                    <div className="flex items-center gap-2">
                      <UserAvatar name={name} userId={m.user_id} size="md" />
                      <div className="min-w-0">
                        <div className="truncate text-sm font-medium">{name}</div>
                        <div className="text-xs text-text-muted capitalize">{m.role}</div>
                      </div>
                    </div>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 shrink-0 text-text-muted hover:text-rose-500"
                      aria-label={t('projectMembers.remove', { name })}
                      onClick={() => remove.mutate(m.user_id)}
                      disabled={remove.isPending}
                    >
                      <X className="h-4 w-4" />
                    </Button>
                  </div>
                )
              })}
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
