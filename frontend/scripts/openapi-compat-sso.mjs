import { spawnSync } from 'node:child_process'
import { resolve } from 'node:path'

// These removals intentionally close local user management and URL-borne tokens.
const approvedRemovals = new Set([
  'removed path /api/v1/admin/users',
  'removed path /api/v1/admin/users/{id}/status',
  'GET /api/v1/events removed parameter query:access_token',
  'removed schema components.schemas.AdminCreateUserRequest',
  'removed schema components.schemas.AdminUserListResponse',
  'removed schema components.schemas.AdminUserResponse',
  'removed schema components.schemas.UpdateUserStatusRequest',
])

const checker = resolve('node_modules/@sdlc/ui/scripts/sdlc-openapi-compat.mjs')
const result = spawnSync(process.execPath, [checker, '--base-ref', 'origin/main'], {
  cwd: process.cwd(),
  encoding: 'utf8',
  env: {
    ...process.env,
    SPEC: 'openapi/openapi.json',
    SPEC_NAME: 'openapi/openapi.json',
  },
})

if (result.error) throw result.error
if (result.status === 0) {
  process.stdout.write(result.stdout)
  process.exit(0)
}

const lines = result.stderr.split(/\r?\n/)
const problems = lines.filter((line) => line.startsWith('  - ')).map((line) => line.slice(4))
if (
  result.status === 1 &&
  result.stderr.includes('OpenAPI compatibility check failed:') &&
  problems.length > 0 &&
  problems.every((problem) => approvedRemovals.has(problem))
) {
  process.stdout.write(`Approved SSO contract removals: ${problems.join('; ')}\n`)
  process.exit(0)
}

process.stdout.write(result.stdout)
process.stderr.write(result.stderr)
process.exit(result.status ?? 1)
