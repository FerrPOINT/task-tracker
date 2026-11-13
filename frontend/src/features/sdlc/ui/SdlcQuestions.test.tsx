import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { ThemeProvider } from '@pdlc/ui/lib'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  answerSdlcQuestion,
  getSdlcQuestions,
  SdlcRequestError,
  type SdlcContext,
} from '@/api/sdlc'
import { lifecycleContext, lifecycleTask, ownerQuestion } from '@/api/test-fixtures/lifecycle'
import i18n from '@/shared/i18n/config'
import { SdlcQuestions } from './SdlcQuestions'

vi.mock('@/api/sdlc', async (original) => ({
  ...(await original<typeof import('@/api/sdlc')>()),
  getSdlcQuestions: vi.fn(),
  answerSdlcQuestion: vi.fn(),
}))
const context: SdlcContext = {
  ...lifecycleContext,
  stage: 'Clarification',
  status: null,
  requirement_revision: 2,
  permissions: { can_answer: true, can_confirm: false },
}
const question = ownerQuestion()
const option = {
  id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
  label: 'Pickup',
  consequences: 'No shipping fee',
}
const changed = vi.fn<() => Promise<void>>()
function mount(current = context) {
  return render(
    <ThemeProvider>
      <QueryClientProvider
        client={
          new QueryClient({
            defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
          })
        }
      >
        <SdlcQuestions
          context={current}
          queryKey={['sdlc', lifecycleTask, 'owner']}
          enabled
          onChanged={changed}
        />
      </QueryClientProvider>
    </ThemeProvider>,
  )
}
describe('owner answers through the existing SDLC contract', () => {
  beforeEach(async () => {
    vi.clearAllMocks()
    await i18n.changeLanguage('en')
    changed.mockResolvedValue()
    vi.mocked(getSdlcQuestions).mockResolvedValue([structuredClone(question)])
    vi.mocked(answerSdlcQuestion).mockImplementation(async (_context, q, command) => ({
      id: option.id,
      question_id: q.id,
      question_version: command.expected_question_version,
      requirement_revision: command.requirement_revision,
      selected_option_ids: command.selected_option_ids,
      text: command.text,
      author_subject: 'owner',
      created_at: '2026-11-12T10:01:00Z',
    }))
  })
  afterEach(cleanup)
  it('submits the original question version and revision without automatic retries', async () => {
    mount()
    const button = await screen.findByRole('button', { name: 'Send answer' })
    expect(button).toBeDisabled()
    fireEvent.change(screen.getByRole('textbox', { name: 'Your answer' }), {
      target: { value: 'Courier delivery' },
    })
    fireEvent.click(button)
    await waitFor(() => expect(answerSdlcQuestion).toHaveBeenCalledOnce())
    expect(answerSdlcQuestion).toHaveBeenCalledWith(context, question, {
      expected_question_version: 4,
      requirement_revision: 2,
      text: 'Courier delivery',
      selected_option_ids: [],
      idempotency_key: expect.any(String),
    })
    await waitFor(() => expect(changed).toHaveBeenCalledOnce())
  })
  it.each(['single', 'multiple'] as const)(
    'preserves %s option IDs when answering',
    async (mode) => {
      vi.mocked(getSdlcQuestions).mockResolvedValue([{ ...question, mode, options: [option] }])
      mount()
      fireEvent.click(
        await screen.findByRole(mode === 'single' ? 'radio' : 'checkbox', { name: /Pickup/ }),
      )
      fireEvent.click(screen.getByRole('button', { name: 'Send answer' }))
      await waitFor(() => expect(answerSdlcQuestion).toHaveBeenCalledOnce())
      expect(vi.mocked(answerSdlcQuestion).mock.calls[0]?.[2].selected_option_ids).toEqual([
        option.id,
      ])
    },
  )
  it('requires text for a custom option', async () => {
    vi.mocked(getSdlcQuestions).mockResolvedValue([
      { ...question, mode: 'single', options: [{ ...option, is_custom: true }] },
    ])
    mount()
    fireEvent.click(await screen.findByRole('radio', { name: /Pickup/ }))
    expect(screen.getByRole('button', { name: 'Send answer' })).toBeDisabled()
    fireEvent.change(screen.getByRole('textbox', { name: 'Your answer' }), {
      target: { value: 'Delivery to office' },
    })
    expect(screen.getByRole('button', { name: 'Send answer' })).toBeEnabled()
  })
  it('does not send a question from a different requirements revision', async () => {
    mount({ ...context, requirement_revision: 3 })
    expect(await screen.findByRole('button', { name: 'Send answer' })).toBeDisabled()
    expect(answerSdlcQuestion).not.toHaveBeenCalled()
  })
  it('does not load or answer owner questions without permission', () => {
    mount({ ...context, permissions: { can_answer: false, can_confirm: false } })
    expect(getSdlcQuestions).not.toHaveBeenCalled()
    expect(screen.queryByRole('button', { name: 'Send answer' })).not.toBeInTheDocument()
  })
  it('keeps an uncertain answer as an error and refreshes before another decision', async () => {
    vi.mocked(answerSdlcQuestion).mockRejectedValue(new SdlcRequestError(503))
    mount()
    fireEvent.change(await screen.findByRole('textbox', { name: 'Your answer' }), {
      target: { value: 'Courier' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Send answer' }))
    expect(await screen.findByRole('alert')).toBeVisible()
    expect(answerSdlcQuestion).toHaveBeenCalledOnce()
    expect(changed).toHaveBeenCalledOnce()
  })
  it('reports a failed read without inventing an empty successful list', async () => {
    vi.mocked(getSdlcQuestions).mockRejectedValue(new SdlcRequestError(403))
    mount()
    expect(await screen.findByRole('alert')).toBeVisible()
    expect(screen.queryByRole('button', { name: 'Send answer' })).not.toBeInTheDocument()
  })
})
