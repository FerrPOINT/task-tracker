import { afterEach, describe, expect, it } from 'vitest'
import { createProductRouter } from '@pdlc/ui/platform'

afterEach(() => { window.history.replaceState(null, '', '/') })
describe('standalone product deployment paths', () => {
  for (const prefix of ['/', '/tracker/']) {
    it(`keeps navigation inside ${prefix}`, async () => {
      window.history.replaceState(null, '', prefix)
      const router = createProductRouter([{ path: '/', element: null }, { path: '/projects', element: null }], {
        serviceKey: 'task-tracker', basename: prefix,
      })
      await router.navigate('/projects')
      expect(window.location.pathname).toBe(prefix + 'projects')
      router.dispose()
    })
  }
  it('preserves root deployment when the option is absent', async () => {
    const router = createProductRouter([{ path: '/', element: null }, { path: '/projects', element: null }], { serviceKey: 'task-tracker' })
    await router.navigate('/projects')
    expect(window.location.pathname).toBe('/projects')
    router.dispose()
  })
})
