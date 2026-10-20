import { z } from 'zod'
import { api } from './client'
import type { components } from './generated'
import {
  nonNilUuid,
  spaceRefSchema,
  spaceKey,
  validitySchema,
  validityIsCurrent,
  spaceCatalogKeys,
  SpaceRequestError,
  type CatalogIdentity,
  type SpaceRef,
} from './space-catalog'

export type VisibleProjectCatalog = components['schemas']['VisibleProjectCatalogV2']
export interface VisibleProjectQuery {
  limit?: number
  offset?: number
  q?: string
  space?: SpaceRef | null
  projectKey?: string
}
const querySchema = z
  .object({
    limit: z.number().int().safe().min(1).max(100).default(50),
    offset: z.number().int().safe().nonnegative().default(0),
    q: z
      .string()
      .refine((value) => Array.from(value).length <= 200)
      .default(''),
    space: spaceRefSchema.nullable().optional(),
    projectKey: z
      .string()
      .regex(/^[A-Za-z0-9-]{1,10}$/)
      .optional(),
  })
  .strict()
const projectSchema = z
  .object({
    id: nonNilUuid,
    key: z.string().regex(/^[A-Za-z0-9-]{1,10}$/),
    name: z.string(),
    description: z.string().nullable().optional(),
    owner_id: nonNilUuid,
    owner_name: z.string(),
    todo_count: z.number().int().safe().nonnegative(),
    in_progress_count: z.number().int().safe().nonnegative(),
    done_count: z.number().int().safe().nonnegative(),
  })
  .strict()
const catalogSchema = z
  .object({
    schema_version: z.literal(2),
    tracker_instance_id: nonNilUuid,
    identity: z.object({ issuer: z.string().min(1), subject: z.string().min(1) }).strict(),
    policy_revision: z.number().int().safe().nonnegative(),
    validity: validitySchema,
    space: spaceRefSchema.nullable(),
    total: z.number().int().safe().nonnegative(),
    offset: z.number().int().safe().nonnegative(),
    limit: z.number().int().safe().min(1).max(100),
    has_more: z.boolean(),
    projects: z.array(projectSchema).max(100),
  })
  .strict()
function sameSpace(a: SpaceRef | null, b: SpaceRef | null): boolean {
  return a === null ? b === null : b !== null && spaceKey(a) === spaceKey(b)
}
export const visibleProjectKeys = {
  identity: (identity: CatalogIdentity) =>
    [...spaceCatalogKeys.identity(identity), 'visible-projects'] as const,
  page: (identity: CatalogIdentity, query: VisibleProjectQuery) =>
    [
      ...visibleProjectKeys.identity(identity),
      query.space ? spaceKey(query.space) : null,
      query.limit ?? 50,
      query.offset ?? 0,
      query.q?.trim() ?? '',
      query.projectKey ?? null,
    ] as const,
}
export async function getVisibleProjectCatalog(
  identity: CatalogIdentity,
  input: VisibleProjectQuery = {},
  signal?: AbortSignal,
): Promise<VisibleProjectCatalog> {
  const query = querySchema.parse(input),
    space = query.space ?? null,
    q = query.q.trim()
  const { data, error, response } = await api.GET('/api/v2/projects', {
    params: {
      query: {
        limit: query.limit,
        offset: query.offset,
        q: q || undefined,
        project_key: query.projectKey,
        registry_instance_id: space?.registry_instance_id,
        space_id: space?.space_id,
      },
    },
    signal,
  })
  if (error || !data) throw new SpaceRequestError(response.status)
  const parsed = catalogSchema.safeParse(data)
  if (!parsed.success) throw new SpaceRequestError(502, 'invalid_response')
  const value = parsed.data
  if (
    value.identity.issuer !== identity.issuer ||
    value.identity.subject !== identity.subject ||
    !sameSpace(value.space, space) ||
    !validityIsCurrent(value.validity) ||
    value.limit !== query.limit ||
    value.offset !== query.offset ||
    value.projects.length > value.limit ||
    (query.projectKey !== undefined && value.projects.some((p) => p.key !== query.projectKey)) ||
    new Set(value.projects.map((p) => p.id.toLowerCase())).size !== value.projects.length ||
    new Set(value.projects.map((p) => p.key)).size !== value.projects.length ||
    value.has_more !== value.offset + value.projects.length < value.total ||
    (value.offset < value.total &&
      value.projects.length !== Math.min(value.limit, value.total - value.offset)) ||
    (value.offset >= value.total && value.projects.length !== 0)
  )
    throw new SpaceRequestError(502, 'invalid_response')
  return value
}
