import { useId, useLayoutEffect, useState } from 'react'
import { Button, Checkbox, ErrorState, Input, LoadingState } from '@pdlc/ui/ui'
import { spaceKey, type SpaceRef } from '@/api/space-catalog'
import { useVisibleSpaces } from '@/shared/api/useVisibleSpaces'

const stateLabels = {
  active: 'Активно',
  archived: 'В архиве',
  provisioning: 'Подготавливается',
  archiving: 'Архивируется',
  restoring: 'Восстанавливается',
}
export function SpaceSetField({
  value,
  onChange,
  disabled = false,
  keepNonEmpty = false,
  onAvailabilityChange,
}: {
  value: SpaceRef[]
  onChange: (next: SpaceRef[]) => void
  disabled?: boolean
  keepNonEmpty?: boolean
  onAvailabilityChange?: (ready: boolean) => void
}) {
  const id = useId()
  const [search, setSearch] = useState('')
  const [offset, setOffset] = useState(0)
  const catalog = useVisibleSpaces({ limit: 10, offset, q: search })
  const ready = Boolean(catalog.data)
  useLayoutEffect(() => {
    onAvailabilityChange?.(ready)
  }, [ready, onAvailabilityChange])
  const selected = new Set(value.map(spaceKey))
  const rows = catalog.data?.spaces ?? []
  function toggle(ref: SpaceRef) {
    if (disabled || !ready) return
    const key = spaceKey(ref)
    if (selected.has(key)) {
      if (keepNonEmpty && value.length === 1) return
      onChange(value.filter((entry) => spaceKey(entry) !== key))
    } else if (rows.some((entry) => spaceKey(entry.space) === key && entry.state === 'active')) {
      onChange([...value, ref])
    }
  }
  return (
    <fieldset className="space-y-3 border-0 p-0" aria-busy={catalog.isFetching || undefined}>
      <legend className="text-sm font-medium">Пространства проекта</legend>
      <p className="text-sm text-text-muted">
        Проекту нужно хотя бы одно пространство. Новые связи добавляются с активными пространствами.
      </p>
      <label htmlFor={id} className="text-sm">
        Поиск пространств
      </label>
      <Input
        id={id}
        type="search"
        value={search}
        onChange={(event) => {
          setSearch(event.target.value)
          setOffset(0)
        }}
        onKeyDown={(event) => {
          if (event.key === 'Enter') event.preventDefault()
        }}
      />
      {!catalog.data && catalog.isFetching ? (
        <LoadingState />
      ) : catalog.unavailable ? (
        <ErrorState
          message="Не удалось подтвердить доступные пространства."
          onRetry={() => void catalog.refetch()}
        />
      ) : (
        <div className="space-y-2">
          {!rows.length && <p className="text-sm text-text-muted">Пространства не найдены.</p>}
          {rows.map((entry) => {
            const checked = selected.has(spaceKey(entry.space))
            return (
              <label
                key={spaceKey(entry.space)}
                className="flex items-start gap-3 rounded-md border border-border p-3"
              >
                <Checkbox
                  className="mt-1 shrink-0"
                  checked={checked}
                  disabled={
                    disabled ||
                    (!checked && entry.state !== 'active') ||
                    (checked && keepNonEmpty && value.length === 1)
                  }
                  onCheckedChange={() => toggle(entry.space)}
                />
                <span className="min-w-0">
                  <span className="block break-words text-sm font-medium">
                    {entry.metadata.name}
                  </span>
                  <span className="block break-all text-xs text-text-muted">
                    {entry.metadata.slug} · {stateLabels[entry.state]} · {entry.space.space_id}
                  </span>
                </span>
              </label>
            )
          })}
          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              variant="outline"
              disabled={offset === 0 || catalog.isFetching}
              onClick={() => setOffset(Math.max(0, offset - 10))}
            >
              Назад
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={!catalog.data?.has_more || catalog.isFetching}
              onClick={() => setOffset(offset + 10)}
            >
              Далее
            </Button>
            <span className="text-sm text-text-muted">Найдено: {catalog.data?.total ?? 0}</span>
          </div>
        </div>
      )}
      <div className="space-y-2" aria-label="Выбранные пространства">
        <p className="text-sm font-medium">Выбрано: {value.length}</p>
        {value.map((ref) => {
          const entry = rows.find((item) => spaceKey(item.space) === spaceKey(ref))
          return (
            <div
              key={spaceKey(ref)}
              className="flex min-w-0 items-center justify-between gap-2 rounded-md border border-border p-2"
            >
              <span className="min-w-0 break-all text-sm">
                {entry?.metadata.name ?? 'Пространство'} · {ref.registry_instance_id}/{ref.space_id}
              </span>
              <Button
                type="button"
                variant="ghost"
                disabled={disabled || !ready || (keepNonEmpty && value.length === 1)}
                aria-label={'Убрать пространство ' + ref.space_id}
                onClick={() => toggle(ref)}
              >
                Убрать
              </Button>
            </div>
          )
        })}
      </div>
    </fieldset>
  )
}
