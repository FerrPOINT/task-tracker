import { expect, it } from 'vitest'
import i18n from 'i18next'

it('retains DOM matchers, translations and storage for browser units by default', () => {
  const element = document.createElement('div')
  document.body.appendChild(element)
  try {
    expect(element).toBeInTheDocument()
    expect(i18n.isInitialized).toBe(true)
    expect(i18n.t('sdlc.loading')).toBe('Загрузка состояния SDLC')
    localStorage.setItem('browser-unit-control', 'ready')
    expect(window.localStorage).toBe(globalThis.localStorage)
    expect(window.localStorage.getItem('browser-unit-control')).toBe('ready')
  } finally {
    element.remove()
    localStorage.removeItem('browser-unit-control')
  }
})
