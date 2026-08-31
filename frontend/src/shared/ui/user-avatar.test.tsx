import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { UserAvatar } from './user-avatar'

describe('UserAvatar', () => {
  it('renders a stable generated image for an assigned user', () => {
    const { rerender } = render(<UserAvatar name="Alice" userId="user-1" />)
    const firstSource = screen.getByRole('img', { name: 'Alice' }).getAttribute('src')

    rerender(<UserAvatar name="Alice" userId="user-1" />)

    expect(firstSource).toContain('data:image/svg+xml')
    expect(screen.getByRole('img', { name: 'Alice' })).toHaveAttribute('src', firstSource)
  })

  it('renders a neutral person icon for an unassigned issue', () => {
    render(<UserAvatar />)

    expect(screen.getByLabelText('Unassigned')).toBeInTheDocument()
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
  })
})
