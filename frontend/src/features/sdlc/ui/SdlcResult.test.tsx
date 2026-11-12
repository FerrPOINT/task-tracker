import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { ThemeProvider } from '@pdlc/ui/lib'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { getSdlcResult, SdlcRequestError } from '@/api/sdlc'
import { lifecycleTask, acceptedResult, resultId } from '@/api/test-fixtures/lifecycle'
import i18n from '@/shared/i18n/config'
import { SdlcResult } from './SdlcResult'

vi.mock('@/api/sdlc', async (original) => ({
  ...(await original<typeof import('@/api/sdlc')>()),
  getSdlcResult: vi.fn(),
}))
function mount(enabled = true) {
  const client = new QueryClient()
  const content = (allowed: boolean) => (
    <ThemeProvider>
      <QueryClientProvider client={client}>
        <SdlcResult
          issueId={lifecycleTask}
          resultId={resultId}
          queryKey={['sdlc', lifecycleTask, 'human']}
          enabled={allowed}
        />
      </QueryClientProvider>
    </ThemeProvider>
  )
  return { ...render(content(enabled)), client, content }
}
describe('accepted result readback', () => {
  beforeEach(async () => {
    vi.clearAllMocks()
    await i18n.changeLanguage('en')
    vi.mocked(getSdlcResult).mockResolvedValue(acceptedResult())
  })
  afterEach(cleanup)
  it('reads one accepted result only after an explicit action and keeps diagnostics collapsed', async () => {
    mount()
    expect(getSdlcResult).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'View accepted result' }))
    expect(await screen.findByText('Review approved')).toBeVisible()
    expect(
      screen.getByText(
        'The stage result was accepted. This does not confirm completion of the entire task.',
      ),
    ).toBeVisible()
    expect(screen.getByLabelText('JSON')).toHaveTextContent(resultId)
    expect(screen.getByLabelText('JSON')).not.toBeVisible()
    expect(getSdlcResult).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: 'Hide result' }))
    expect(screen.queryByRole('region', { name: 'Accepted stage result' })).not.toBeInTheDocument()
    await waitFor(() => expect(getSdlcResult).toHaveBeenCalledOnce())
  })
  it('hides a cached result after the parent loses admission', async () => {
    const view = mount()
    fireEvent.click(screen.getByRole('button', { name: 'View accepted result' }))
    await screen.findByText('Review approved')
    expect(view.client.getQueryData(['sdlc', lifecycleTask, 'human', 'result', resultId])).toEqual(
      acceptedResult(),
    )
    view.rerender(view.content(false))
    expect(screen.queryByText('Review approved')).not.toBeInTheDocument()
  })
  it('shows access denial without rendering stale cached data', async () => {
    const view = mount()
    fireEvent.click(screen.getByRole('button', { name: 'View accepted result' }))
    await screen.findByText('Review approved')
    vi.mocked(getSdlcResult).mockRejectedValue(new SdlcRequestError(403))
    await view.client.invalidateQueries({
      queryKey: ['sdlc', lifecycleTask, 'human', 'result', resultId],
    })
    expect(await screen.findByRole('alert')).toBeVisible()
    expect(screen.queryByText('Review approved')).not.toBeInTheDocument()
  })
})
