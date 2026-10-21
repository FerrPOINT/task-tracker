import { z } from 'zod'
import { api } from './client'
import type { components, paths } from './generated'

export type SpaceRef = components['schemas']['SpaceRef']
export type CatalogIdentity = components['schemas']['CentralIdentity']
export type VisibleSpaceCatalog = components['schemas']['VisibleSpaceCatalogV2']
export type VisibleSpaceQuery = NonNullable<
  paths['/api/v2/spaces/visible']['get']['parameters']['query']
>
export const nonNilUuid = z
  .string()
  .uuid()
  .refine((v) => v !== '00000000-0000-0000-0000-000000000000')
export const spaceRefSchema = z
  .object({
    registry_instance_id: nonNilUuid,
    space_id: nonNilUuid,
  })
  .strict()
const integer = z.number().int().safe()
export const validitySchema = z
  .object({
    observed_at: integer.positive(),
    valid_until: integer.positive(),
  })
  .strict()
const identitySchema = z.object({ issuer: z.string().url(), subject: z.string().min(1) }).strict()
const querySchema = z
  .object({
    limit: integer.min(1).max(10).default(10),
    offset: integer.min(0).default(0),
    q: z
      .string()
      .refine((s) => Array.from(s).length <= 200)
      .default(''),
  })
  .strict()
const pageSchema = z
  .object({
    schema_version: z.literal(2),
    identity: identitySchema,
    policy_revision: integer.min(0),
    validity: validitySchema,
    spaces: z
      .array(
        z
          .object({
            space: spaceRefSchema,
            revision: integer.positive(),
            state: z.enum(['active', 'archived', 'provisioning', 'archiving', 'restoring']),
            metadata: z
              .object({
                slug: z.string().min(1),
                name: z.string().refine((v) => v.trim().length > 0),
                description: z.string(),
                responsible_subject: z.string().refine((v) => v.trim().length > 0),
              })
              .strict(),
          })
          .strict(),
      )
      .max(10),
    total: integer.min(0),
    limit: integer.min(1).max(10),
    offset: integer.min(0),
    has_more: z.boolean(),
  })
  .strict()

export class SpaceRequestError extends Error {
  constructor(
    public readonly status: number,
    public readonly kind: 'http' | 'invalid_response' = 'http',
  ) {
    super(kind === 'invalid_response' ? 'Unconfirmed space response' : 'Space request failed')
    this.name = 'SpaceRequestError'
  }
}
export function spaceKey(space: SpaceRef): string {
  return (space.registry_instance_id + '/' + space.space_id).toLowerCase()
}
export function validityIsCurrent(
  value: components['schemas']['AdmissionValidity'],
  now = Math.floor(Date.now() / 1000),
): boolean {
  return (
    Number.isSafeInteger(value.observed_at) &&
    Number.isSafeInteger(value.valid_until) &&
    value.observed_at > 0 &&
    value.observed_at <= now &&
    value.valid_until > now &&
    value.valid_until > value.observed_at &&
    value.valid_until - value.observed_at <= 60
  )
}
export function catalogIsCurrent(page: VisibleSpaceCatalog, identity: CatalogIdentity): boolean {
  return (
    page.identity.issuer === identity.issuer &&
    page.identity.subject === identity.subject &&
    validityIsCurrent(page.validity)
  )
}
export const spaceCatalogKeys = {
  identity: (identity: CatalogIdentity) =>
    ['visible-spaces-v2', identity.issuer, identity.subject] as const,
  page: (identity: CatalogIdentity, query: VisibleSpaceQuery = {}) => {
    const normalized = querySchema.parse(query)
    return [
      ...spaceCatalogKeys.identity(identity),
      normalized.limit,
      normalized.offset,
      normalized.q,
    ] as const
  },
}
export async function getVisibleSpaceCatalog(
  identity: CatalogIdentity,
  query: VisibleSpaceQuery = {},
  signal?: AbortSignal,
): Promise<VisibleSpaceCatalog> {
  identitySchema.parse(identity)
  const normalized = querySchema.parse(query)
  const { data, error, response } = await api.GET('/api/v2/spaces/visible', {
    params: { query: normalized },
    signal,
  })
  if (error || !data) throw new SpaceRequestError(response.status)
  const parsed = pageSchema.safeParse(data)
  if (!parsed.success) throw new SpaceRequestError(502, 'invalid_response')
  const page = parsed.data
  const seen = new Set(page.spaces.map((entry) => spaceKey(entry.space)))
  if (
    !catalogIsCurrent(page, identity) ||
    page.limit !== normalized.limit ||
    page.offset !== normalized.offset ||
    page.spaces.length !==
      Math.min(normalized.limit, Math.max(page.total - normalized.offset, 0)) ||
    page.has_more !== normalized.offset + page.spaces.length < page.total ||
    (page.policy_revision === 0 && page.total !== 0) ||
    seen.size !== page.spaces.length
  )
    throw new SpaceRequestError(502, 'invalid_response')
  return page
}
