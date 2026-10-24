// @vitest-environment node

import { expect, it } from 'vitest'
import i18n from 'i18next'

it('keeps pure Node units free of browser setup', () => {
  expect(typeof window).toBe('undefined')
  expect(typeof document).toBe('undefined')
  expect('toBeInTheDocument' in expect(null)).toBe(false)
  expect(i18n.isInitialized).not.toBe(true)
  expect(globalThis).not.toHaveProperty('localStorage')
  expect(globalThis).not.toHaveProperty('sessionStorage')
})
