import { memo } from 'react'
import { createAvatar, type Style } from '@dicebear/core'
import * as micah from '@dicebear/micah'
import { UserRound } from 'lucide-react'

type UserAvatarProps = {
  name?: string | null
  userId?: string | null
  size?: 'sm' | 'md'
}

const sizeClasses = {
  sm: 'h-6 w-6',
  md: 'h-8 w-8',
}

const micahStyle: Style<micah.Options> = {
  create: micah.create,
  meta: micah.meta,
  schema: micah.schema,
}

function avatarSource(seed: string) {
  return createAvatar(micahStyle, {
    backgroundColor: ['e2e8f0'],
    radius: 50,
    seed,
    size: 64,
  }).toDataUri()
}

function UserAvatarInner({ name, userId, size = 'sm' }: UserAvatarProps) {
  const className = `${sizeClasses[size]} shrink-0 rounded-full`
  const seed = userId || name

  if (!seed) {
    return (
      <span
        aria-label="Unassigned"
        className={`flex ${className} items-center justify-center bg-slate-200 text-slate-500 dark:bg-slate-700 dark:text-slate-300`}
      >
        <UserRound className="h-3.5 w-3.5" aria-hidden="true" />
      </span>
    )
  }

  return <img alt={name || 'User'} className={className} src={avatarSource(seed)} />
}

export const UserAvatar = memo(UserAvatarInner)
