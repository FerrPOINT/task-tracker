import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, expect, it } from 'vitest'
import { CommentBody } from './CommentBody'

afterEach(cleanup)

it('renders fenced code through a named keyboard-accessible Base region', async () => {
  const user = userEvent.setup()
  render(<CommentBody body={'```sh\ncommand --namespace=local\n<script>literal</script>\n```'} />)
  const code = screen.getByRole('region', { name: 'Код' })
  await user.tab()
  expect(code).toHaveFocus()
  expect(code).toHaveTextContent('command --namespace=local')
  expect(code).toHaveTextContent('<script>literal</script>')
  expect(code).toHaveAttribute('data-wrap', 'false')
  expect(code.querySelector('script')).toBeNull()
})

it('preserves prose, inline code and Markdown link policy without interpreting raw HTML', () => {
  const page = render(
    <CommentBody
      body={
        '**Ready**\nSecond line with `issue_id`\n\n[Source](/issues/42) [Unsafe](javascript:alert(1))\n\n<div>Hidden HTML</div>'
      }
    />,
  )
  expect(screen.getByText('Ready').tagName).toBe('STRONG')
  expect(screen.getByText('issue_id').tagName).toBe('CODE')
  expect(screen.getByRole('link', { name: 'Source' })).toHaveAttribute('href', '/issues/42')
  expect(screen.getByText('Unsafe')).toHaveAttribute('href', '')
  expect(screen.queryByText('Hidden HTML')).not.toBeInTheDocument()
  expect(page.container.querySelector('pre')).toBeNull()
  expect(page.container.querySelector('[data-preserve-line-breaks]')).toHaveAttribute(
    'data-preserve-line-breaks',
    'true',
  )
})

it('updates one comment without retaining code from the previous body', () => {
  const page = render(<CommentBody body={'```\nOriginal code\n```'} />)
  expect(screen.getByRole('region', { name: 'Код' })).toHaveTextContent('Original code')
  page.rerender(<CommentBody body="Updated comment" />)
  expect(screen.queryByRole('region')).not.toBeInTheDocument()
  expect(screen.queryByText('Original code')).not.toBeInTheDocument()
  expect(screen.getByText('Updated comment')).toBeInTheDocument()
})

it('keeps the code node and focus when the same comment is rendered again', () => {
  const body = '```\nRetain code focus\n```'
  const page = render(<CommentBody body={body} />)
  const code = screen.getByRole('region', { name: 'Код' })
  code.focus()
  page.rerender(<CommentBody body={body} />)
  expect(screen.getByRole('region', { name: 'Код' })).toBe(code)
  expect(code).toHaveFocus()
})
