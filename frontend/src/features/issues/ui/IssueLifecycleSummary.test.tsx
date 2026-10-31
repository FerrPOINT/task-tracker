import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { IssueLifecycleSummary } from './IssueLifecycleSummary'

describe('common issue lifecycle summary', () => {
  it('keeps an intermediate ready task distinct from final completion', () => {
    render(
      <IssueLifecycleSummary
        lifecycle={{ stage: 'Analysis', status: 'Ready', revision: 3 }}
        legacyStatus="Unknown"
      />,
    )
    expect(screen.getByText('Анализ')).toBeVisible()
    expect(screen.getByText('Готова к работе')).toBeVisible()
    expect(screen.queryByText('Unknown')).not.toBeInTheDocument()
    expect(screen.queryByText('Завершена')).not.toBeInTheDocument()
  })
  it('shows deployed completion and preserves the legacy read-only label before activation', () => {
    const view = render(
      <IssueLifecycleSummary
        lifecycle={{ stage: 'Deployment', status: 'Completed', revision: 7 }}
      />,
    )
    expect(screen.getByText('Развёртывание')).toBeVisible()
    expect(screen.getByText('Завершена')).toBeVisible()
    view.rerender(<IssueLifecycleSummary legacyStatus="Custom old label" />)
    expect(screen.getByText('Custom old label')).toBeVisible()
    expect(screen.queryByText('Развёртывание')).not.toBeInTheDocument()
  })
})
