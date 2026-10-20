import { z } from 'zod'
import { api } from './client'
import type { components } from './generated'
import type { Project } from './project'
import {
  nonNilUuid,
  spaceRefSchema,
  spaceKey,
  validitySchema,
  validityIsCurrent,
  SpaceRequestError,
  spaceCatalogKeys,
  type CatalogIdentity,
} from './space-catalog'

export type ConfirmedProjectSpaces = components['schemas']['ConfirmedProjectSpaces']
export type ReplaceProjectSpaces = components['schemas']['ReplaceProjectSpaces']
export type CreateProjectInSpaces = components['schemas']['CreateProjectInSpaces']
export type ProjectIdentity = Pick<Project, 'key' | 'id'>
const keySchema = z.string().regex(/^[A-Za-z0-9-]{1,10}$/)
const spacesSchema = z
  .array(spaceRefSchema)
  .min(1)
  .max(1000)
  .refine((spaces) => new Set(spaces.map(spaceKey)).size === spaces.length)
const createSchema = z
  .object({
    key: keySchema,
    name: z.string().refine((s) => s.trim().length > 0 && Array.from(s.trim()).length <= 200),
    description: z.string().nullable().optional(),
    spaces: spacesSchema,
  })
  .strict()
export const projectSpaceChangeSchema = z
  .object({
    operation_id: nonNilUuid,
    expected_revision: z.number().int().safe().positive(),
    spaces: spacesSchema,
  })
  .strict()
const linksSchema = z
  .object({
    project: z.object({ tracker_instance_id: nonNilUuid, project_id: nonNilUuid }).strict(),
    revision: z.number().int().safe().positive(),
    spaces: spacesSchema,
    validity: validitySchema,
  })
  .strict()
function projectIdentity(project: ProjectIdentity): void {
  keySchema.parse(project.key)
  nonNilUuid.parse(project.id)
}
function checkedLinks(value: unknown, project: ProjectIdentity): ConfirmedProjectSpaces {
  const result = linksSchema.safeParse(value)
  if (
    !result.success ||
    result.data.project.project_id.toLowerCase() !== project.id.toLowerCase() ||
    !validityIsCurrent(result.data.validity)
  )
    throw new SpaceRequestError(502, 'invalid_response')
  return result.data
}
export const projectSpaceKeys = {
  project: (identity: CatalogIdentity, project: ProjectIdentity) =>
    [...spaceCatalogKeys.identity(identity), 'project-spaces', project.id] as const,
}
export async function getProjectSpaces(
  project: ProjectIdentity,
  signal?: AbortSignal,
): Promise<ConfirmedProjectSpaces> {
  projectIdentity(project)
  const { data, error, response } = await api.GET('/api/v2/projects/{project_key}/spaces', {
    params: { path: { project_key: project.key } },
    signal,
  })
  if (error || !data) throw new SpaceRequestError(response.status)
  return checkedLinks(data, project)
}
export async function replaceProjectSpaces(
  project: ProjectIdentity,
  change: ReplaceProjectSpaces,
): Promise<ConfirmedProjectSpaces> {
  projectIdentity(project)
  const body = projectSpaceChangeSchema.parse(change)
  const { data, error, response } = await api.PUT('/api/v2/projects/{project_key}/spaces', {
    params: { path: { project_key: project.key } },
    body,
  })
  if (error || !data) throw new SpaceRequestError(response.status)
  // An original-key replay returns the actual current set, possibly superseded.
  // Never relabel that response as the requested set or generate a retry key here.
  return checkedLinks(data, project)
}
export async function createProjectInSpaces(input: CreateProjectInSpaces): Promise<Project> {
  const body = createSchema.parse(input)
  const { data, error, response } = await api.POST('/api/v2/projects', { body })
  if (error || !data) throw new SpaceRequestError(response.status)
  if (!nonNilUuid.safeParse(data.id).success || data.key !== body.key)
    throw new SpaceRequestError(502, 'invalid_response')
  return data
}

export type ProjectSpaceProjection = components['schemas']['ProjectSpaceProjectionStatus']
function checkedProjection(value: unknown, project: ProjectIdentity): ProjectSpaceProjection {
  const parsed = z
    .object({
      operation_id: nonNilUuid,
      project: z.object({ tracker_instance_id: nonNilUuid, project_id: nonNilUuid }).strict(),
      revision: z.number().int().safe().positive(),
      confirmed: z.boolean(),
      last_error: z.string().nullable().optional(),
    })
    .strict()
    .safeParse(value)
  if (!parsed.success || parsed.data.project.project_id.toLowerCase() !== project.id.toLowerCase())
    throw new SpaceRequestError(502, 'invalid_response')
  return parsed.data
}
export async function getProjectSpaceProjection(
  project: ProjectIdentity,
  signal?: AbortSignal,
): Promise<ProjectSpaceProjection> {
  projectIdentity(project)
  const { data, error, response } = await api.GET(
    '/api/v2/projects/{project_key}/spaces/projection',
    {
      params: { path: { project_key: project.key } },
      signal,
    },
  )
  if (error || !data) throw new SpaceRequestError(response.status)
  return checkedProjection(data, project)
}
export async function reconcileProjectSpaceProjection(
  project: ProjectIdentity,
): Promise<ProjectSpaceProjection> {
  projectIdentity(project)
  const { data, error, response } = await api.POST(
    '/api/v2/projects/{project_key}/spaces/projection/reconcile',
    {
      params: { path: { project_key: project.key } },
    },
  )
  if (error || !data) throw new SpaceRequestError(response.status)
  return checkedProjection(data, project)
}

export interface ProjectSpaceManagement {
  links: ConfirmedProjectSpaces
  canChange: boolean
  policyRevision: number
}
export async function getProjectSpaceManagement(
  project: ProjectIdentity,
  signal?: AbortSignal,
): Promise<ProjectSpaceManagement> {
  projectIdentity(project)
  const { data, error, response } = await api.GET('/api/v2/projects/{project_key}/spaces', {
    params: { path: { project_key: project.key } },
    signal,
  })
  if (error || !data) throw new SpaceRequestError(response.status)
  const hint = response.headers.get('x-pdlc-project-spaces-can-change')
  if (hint !== 'true' && hint !== 'false') throw new SpaceRequestError(502, 'invalid_response')
  const revision = response.headers.get('x-pdlc-admission-policy-revision')
  if (
    revision === null ||
    !/^(0|[1-9][0-9]*)$/.test(revision) ||
    !Number.isSafeInteger(Number(revision))
  )
    throw new SpaceRequestError(502, 'invalid_response')
  return {
    links: checkedLinks(data, project),
    canChange: hint === 'true',
    policyRevision: Number(revision),
  }
}

export type ProjectSpaceOperationReceipt = components['schemas']['ProjectSpaceOperationReceipt']
export async function getProjectSpaceOperationReceipt(
  project: ProjectIdentity,
  reference: components['schemas']['ProjectRef'],
  command: ReplaceProjectSpaces,
  signal?: AbortSignal,
): Promise<ProjectSpaceOperationReceipt> {
  projectIdentity(project)
  const original = projectSpaceChangeSchema.parse(command)
  const ref = z
    .object({ tracker_instance_id: nonNilUuid, project_id: nonNilUuid })
    .strict()
    .parse(reference)
  if (ref.project_id.toLowerCase() !== project.id.toLowerCase())
    throw new SpaceRequestError(502, 'invalid_response')
  const { data, error, response } = await api.GET(
    '/api/v2/projects/{project_key}/spaces/operations/{operation_id}',
    {
      params: { path: { project_key: project.key, operation_id: original.operation_id } },
      signal,
    },
  )
  if (error || !data) throw new SpaceRequestError(response.status)
  const parsed = z
    .object({
      schema_version: z.literal(1),
      project: z.object({ tracker_instance_id: nonNilUuid, project_id: nonNilUuid }).strict(),
      operation_id: nonNilUuid,
      applied: z.literal(true),
      applied_revision: z.number().int().safe().positive(),
      change: projectSpaceChangeSchema,
      validity: validitySchema,
    })
    .strict()
    .safeParse(data)
  if (!parsed.success) throw new SpaceRequestError(502, 'invalid_response')
  const receipt = parsed.data
  if (
    receipt.project.project_id.toLowerCase() !== ref.project_id.toLowerCase() ||
    receipt.project.tracker_instance_id.toLowerCase() !== ref.tracker_instance_id.toLowerCase() ||
    receipt.operation_id.toLowerCase() !== original.operation_id.toLowerCase() ||
    receipt.change.operation_id.toLowerCase() !== original.operation_id.toLowerCase() ||
    receipt.change.expected_revision !== original.expected_revision ||
    receipt.change.spaces.map(spaceKey).join('|') !== original.spaces.map(spaceKey).join('|') ||
    receipt.applied_revision < original.expected_revision ||
    !validityIsCurrent(receipt.validity)
  )
    throw new SpaceRequestError(502, 'invalid_response')
  return receipt
}
