import type { ReactNode } from 'react'
import { ErrorState, RenderBoundary } from '@pdlc/ui/ui'

export function ProjectCommandRecovery({ children }: { children: ReactNode }) {
  return (
    <RenderBoundary
      fallback={
        <ErrorState message="Не удалось открыть форму проекта. Сохранённый журнал не удалён; требуется проверка ошибки." />
      }
    >
      {children}
    </RenderBoundary>
  )
}
