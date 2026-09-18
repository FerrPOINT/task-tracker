import type { TFunction } from 'i18next'

export function statusLabel(name: string, t: TFunction): string {
  const key = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/_+$/, '')
  return t(`status.${key}`, { defaultValue: name })
}
