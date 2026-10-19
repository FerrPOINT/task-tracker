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
const changeSchema = z
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
  const body = changeSchema.parse(change)
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
