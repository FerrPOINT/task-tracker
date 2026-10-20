import { Component, type ReactNode } from 'react'
import { ErrorState } from '@pdlc/ui/ui'

export class ProjectCommandRecovery extends Component<
  { children: ReactNode },
  { broken: boolean }
> {
  state = { broken: false }
  static getDerivedStateFromError() {
    return { broken: true }
  }
  render() {
    return this.state.broken ? (
      <ErrorState message="Не удалось открыть форму проекта. Сохранённый журнал не удалён; требуется проверка ошибки." />
    ) : (
      this.props.children
    )
  }
}
