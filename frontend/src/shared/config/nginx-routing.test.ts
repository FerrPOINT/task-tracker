import { describe, expect, it } from 'vitest'

import nginxConfig from '../../../nginx.conf?raw'

describe('nginx static and API routing', () => {
  it('preserves the frontend origin for the API root', () => {
    expect(nginxConfig).toContain('location = /api')
    expect(nginxConfig).toMatch(/location = \/api\s*\{[\s\S]*proxy_pass http:\/\/backend:3456;/)
  })

  it('returns 404 for missing hashed assets instead of SPA HTML', () => {
    expect(nginxConfig).toMatch(/location \/assets\/\s*\{[\s\S]*try_files \$uri =404;/)
  })
})
