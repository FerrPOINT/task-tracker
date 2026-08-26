import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@/shared/lib/theme'
import i18n from '@/shared/i18n/config'
import { TimeTrackingPanel } from './TimeTrackingPanel'

beforeAll(() => {
  i18n.changeLanguage('en')
})

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

describe('TimeTrackingPanel', () => {
  it('renders time summary with spent and estimated', () => {
    render(wrapper(
      <TimeTrackingPanel
        timeSpentSeconds={3600}
        originalEstimateSeconds={7200}
        remainingEstimateSeconds={3600}
        onLogWork={vi.fn()}
      />,
    ))
    const summary = screen.getByTestId('time-tracking-summary')
    expect(summary).toBeInTheDocument()
    expect(summary.textContent).toMatch(/1h/)
    expect(summary.textContent).toMatch(/2h/)
  })

  it('calls onLogWork when log work button is clicked', () => {
    const onLogWork = vi.fn()
    render(wrapper(
      <TimeTrackingPanel
        timeSpentSeconds={3600}
        originalEstimateSeconds={7200}
        remainingEstimateSeconds={3600}
        onLogWork={onLogWork}
      />,
    ))
    fireEvent.click(screen.getByRole('button', { name: /log work/i }))
    expect(onLogWork).toHaveBeenCalledOnce()
  })
})