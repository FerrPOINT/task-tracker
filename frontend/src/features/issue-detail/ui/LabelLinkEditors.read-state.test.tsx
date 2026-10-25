import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ApiError, ThemeProvider } from '@pdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { LabelEditor } from './LabelEditor'
import { LinkEditor } from './LinkEditor'

const mocks = vi.hoisted(() => ({
  project: vi.fn(),
  issue: vi.fn(),
  links: vi.fn(),
  createLabel: vi.fn(),
  attach: vi.fn(),
  detach: vi.fn(),
  createLink: vi.fn(),
  remove: vi.fn(),
  readProject: vi.fn(),
  readIssue: vi.fn(),
  readLinks: vi.fn(),
}))
vi.mock('@/shared/api/hooks', () => ({
  useProjectLabels: mocks.project,
  useIssueLabels: mocks.issue,
  useIssueLinks: mocks.links,
  useCreateLabel: () => ({ mutateAsync: mocks.createLabel, isPending: false }),
  useAttachLabel: () => ({ mutate: mocks.attach, mutateAsync: mocks.attach, isPending: false }),
  useDetachLabel: () => ({ mutate: mocks.detach, isPending: false }),
  useCreateIssueLink: () => ({ mutateAsync: mocks.createLink, isPending: false }),
  useDeleteIssueLink: () => ({ mutate: mocks.remove, isPending: false }),
}))
const label = { id: 'label-one', name: 'Confirmed label', color: '#22c55e', project_id: 'p1' }
const link = {
  id: 'link-one',
  source_id: 'i1',
  source_key: 'TT-1',
  target_id: 'i2',
  target_key: 'TT-2',
  link_type: 'blocks',
}
const ready = (data: unknown, refetch: () => unknown) => ({
  data,
  error: null,
  isLoading: false,
  isPending: false,
  refetch,
})
type Editor = 'labels' | 'links'
function panel(kind: Editor, issueId = 'i1', projectKey = 'TT') {
  return (
    <MemoryRouter>
      <ThemeProvider theme="light" onThemeChange={() => {}}>
        {kind === 'labels' ? (
          <LabelEditor issueId={issueId} projectKey={projectKey} />
        ) : (
          <LinkEditor issueId={issueId} currentKey="TT-1" />
        )}
      </ThemeProvider>
    </MemoryRouter>
  )
}
function fail(kind: Editor, status: number, data: unknown = undefined) {
  const query = kind === 'labels' ? mocks.issue : mocks.links
  const retry = kind === 'labels' ? mocks.readIssue : mocks.readLinks
  query.mockReturnValue({ ...ready(data, retry), error: new ApiError(status, 'unavailable') })
}
function noWrites() {
  for (const write of [
    mocks.createLabel,
    mocks.attach,
    mocks.detach,
    mocks.createLink,
    mocks.remove,
  ])
    expect(write).not.toHaveBeenCalled()
}
beforeEach(async () => {
  vi.clearAllMocks()
  await i18n.changeLanguage('en')
  mocks.project.mockReturnValue(ready([label], mocks.readProject))
  mocks.issue.mockReturnValue(ready([label], mocks.readIssue))
  mocks.links.mockReturnValue(ready([link], mocks.readLinks))
})
afterEach(cleanup)

it.each(['project', 'issue', 'links'] as const)(
  'does not treat an unread %s resource as empty',
  (resource) => {
    const kind = resource === 'links' ? 'links' : 'labels'
    mocks[resource].mockReturnValue({
      ...ready(undefined, vi.fn()),
      isLoading: true,
      isPending: true,
    })
    render(panel(kind))
    expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'true')
    expect(screen.queryByText(i18n.t(kind + '.none'))).not.toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    noWrites()
  },
)

it.each(['labels', 'links'] as const)(
  'shows confirmed empty %s only after a successful read',
  (kind) => {
    if (kind === 'labels') {
      mocks.project.mockReturnValue(ready([], mocks.readProject))
      mocks.issue.mockReturnValue(ready([], mocks.readIssue))
    } else mocks.links.mockReturnValue(ready([], mocks.readLinks))
    render(panel(kind))
    expect(screen.getByText(i18n.t(kind + '.none'))).toBeVisible()
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
    noWrites()
  },
)

it.each([401, 403, 404, 503])(
  'does not display an initial %s as empty labels or links',
  (status) => {
    for (const kind of ['labels', 'links'] as const) {
      fail(kind, status)
      render(panel(kind))
      expect(screen.queryByText(i18n.t(kind + '.none'))).not.toBeInTheDocument()
      expect(screen.queryByText(kind === 'labels' ? label.name : 'TT-2')).not.toBeInTheDocument()
      if (status === 503) {
        fireEvent.click(screen.getByRole('button', { name: /retry/i }))
        expect(kind === 'labels' ? mocks.readIssue : mocks.readLinks).toHaveBeenCalledOnce()
        if (kind === 'labels') expect(mocks.readProject).toHaveBeenCalledOnce()
      }
      noWrites()
      cleanup()
    }
  },
)

it.each(['labels', 'links'] as const)(
  'keeps a %s draft and confirmed rows read-only after a temporary failure',
  (kind) => {
    const view = render(panel(kind))
    fireEvent.click(
      screen.getByRole('button', { name: kind === 'labels' ? /new label/i : /add link/i }),
    )
    const input = screen.getByTestId(kind === 'labels' ? 'label-name-input' : 'link-target-input')
    fireEvent.change(input, { target: { value: 'Local unsaved value' } })
    fail(kind, 503, kind === 'labels' ? [label] : [link])
    view.rerender(panel(kind))
    expect(screen.getByTestId(kind === 'labels' ? 'label-name-input' : 'link-target-input')).toBe(
      input,
    )
    expect(input).toHaveValue('Local unsaved value')
    expect(input).toBeDisabled()
    expect(screen.getByText(kind === 'labels' ? label.name : 'TT-2')).toBeInTheDocument()
    fireEvent.keyDown(input, { key: 'Enter' })
    noWrites()
    if (kind === 'labels') mocks.issue.mockReturnValue(ready([label], mocks.readIssue))
    else mocks.links.mockReturnValue(ready([link], mocks.readLinks))
    view.rerender(panel(kind))
    expect(input).toBeEnabled()
    expect(input).toHaveValue('Local unsaved value')
  },
)

it.each([401, 403, 404])('hides retained labels and links on %s', (status) => {
  for (const kind of ['labels', 'links'] as const) {
    fail(kind, status, kind === 'labels' ? [label] : [link])
    render(panel(kind))
    expect(screen.queryByText(kind === 'labels' ? label.name : 'TT-2')).not.toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    noWrites()
    cleanup()
  }
})

it('gives catalog access loss priority over a temporary issue-label failure', () => {
  mocks.project.mockReturnValue({
    ...ready([label], mocks.readProject),
    error: new ApiError(403, 'denied'),
  })
  fail('labels', 503, [label])
  render(panel('labels'))
  expect(screen.queryByText(label.name)).not.toBeInTheDocument()
  expect(screen.queryByRole('button', { name: /retry/i })).not.toBeInTheDocument()
  noWrites()
})

it.each(['labels', 'links'] as const)(
  'resets the %s draft when the issue identity changes',
  (kind) => {
    const view = render(panel(kind))
    fireEvent.click(
      screen.getByRole('button', { name: kind === 'labels' ? /new label/i : /add link/i }),
    )
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Foreign issue draft' } })
    view.rerender(panel(kind, 'another-issue'))
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    noWrites()
  },
)
