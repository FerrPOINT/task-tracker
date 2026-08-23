import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router'
import { listProjects, createProject, updateProject, deleteProject } from '@/api/project'
import { getBoard, getBacklog, moveIssue, type MoveIssueInput } from '@/api/board'
import { searchIssues, type SearchFilters } from '@/api/search'
import { login, register, getCurrentUser, listUsers, logout } from '@/api/auth'
import { createIssue } from '@/api/issue-create'
import { updateIssue, deleteIssue } from '@/api/issue'
import { getDashboard } from '@/api/dashboard'
import { useAuthStore } from '@/shared/auth/store'
import {
  listProjectMembers,
  addProjectMember,
  removeProjectMember,
  type AddProjectMemberInput,
} from '@/api/members'

import {
  closeSprint,
  createSprint,
  listSprints,
  moveIssueToSprint,
  removeIssueFromSprint,
  startSprint,
  updateSprint,
  type CreateSprintRequest,
  type UpdateSprintRequest,
} from '@/api/sprint'
import { listStatuses, listTransitions, listIssueTypes } from '@/api/workflow'

export const workflowKeys = {
  statuses: ['statuses'] as const,
  transitions: ['transitions'] as const,
  issueTypes: ['issue-types'] as const,
}

export function useStatuses() {
  return useQuery({ queryKey: workflowKeys.statuses, queryFn: listStatuses, staleTime: 5 * 60 * 1000 })
}

export function useTransitions() {
  return useQuery({ queryKey: workflowKeys.transitions, queryFn: listTransitions, staleTime: 5 * 60 * 1000 })
}

export function useIssueTypes() {
  return useQuery({ queryKey: workflowKeys.issueTypes, queryFn: listIssueTypes, staleTime: 5 * 60 * 1000 })
}

export const projectKeys = {
  all: ['projects'] as const,
  detail: (key: string) => ['project', key] as const,
  sprints: (key: string) => ['sprints', key] as const,
}

export function useSprints(projectKey: string | undefined) {
  return useQuery({
    queryKey: projectKeys.sprints(projectKey ?? ''),
    queryFn: () => listSprints(projectKey!),
    enabled: !!projectKey,
  })
}

export function useCreateSprint(projectKey: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: CreateSprintRequest) => createSprint(projectKey, input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: projectKeys.sprints(projectKey) })
      qc.invalidateQueries({ queryKey: ['backlog', projectKey] })
    },
  })
}

export function useUpdateSprint(projectKey: string, sprintId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: UpdateSprintRequest) => updateSprint(projectKey, sprintId, input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: projectKeys.sprints(projectKey) })
      qc.invalidateQueries({ queryKey: ['backlog', projectKey] })
    },
  })
}

export function useStartSprint(projectKey: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (sprintId: string) => startSprint(projectKey, sprintId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: projectKeys.sprints(projectKey) })
      qc.invalidateQueries({ queryKey: ['backlog', projectKey] })
      qc.invalidateQueries({ queryKey: projectKeys.detail(projectKey) })
    },
  })
}

export function useCloseSprint(projectKey: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (sprintId: string) => closeSprint(projectKey, sprintId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: projectKeys.sprints(projectKey) })
      qc.invalidateQueries({ queryKey: ['backlog', projectKey] })
      qc.invalidateQueries({ queryKey: projectKeys.detail(projectKey) })
    },
  })
}

export function useMoveIssueToSprint(projectKey: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ sprintId, issueId }: { sprintId: string; issueId: string }) =>
      moveIssueToSprint(projectKey, sprintId, issueId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: projectKeys.sprints(projectKey) })
      qc.invalidateQueries({ queryKey: ['backlog', projectKey] })
      qc.invalidateQueries({ queryKey: projectKeys.detail(projectKey) })
    },
  })
}

export function useRemoveIssueFromSprint(projectKey: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ sprintId, issueId }: { sprintId: string; issueId: string }) =>
      removeIssueFromSprint(projectKey, sprintId, issueId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: projectKeys.sprints(projectKey) })
      qc.invalidateQueries({ queryKey: ['backlog', projectKey] })
      qc.invalidateQueries({ queryKey: projectKeys.detail(projectKey) })
    },
  })
}

export function useProjects() {
  return useQuery({
    queryKey: projectKeys.all,
    queryFn: listProjects,
  })
}

export function useBoard(projectKey: string | undefined) {
  return useQuery({
    queryKey: projectKeys.detail(projectKey ?? ''),
    queryFn: () => getBoard(projectKey!),
    enabled: !!projectKey,
  })
}

export function useBacklog(projectKey: string | undefined) {
  return useQuery({
    queryKey: ['backlog', projectKey ?? ''],
    queryFn: () => getBacklog(projectKey!),
    enabled: !!projectKey,
  })
}

export function useIssues(filters: SearchFilters = {}) {
  return useQuery({
    queryKey: ['search', filters],
    queryFn: () => searchIssues(filters),
  })
}

export function useSearch(q: string) {
  return useIssues({ q })
}

export function useDashboard() {
  return useQuery({
    queryKey: ['dashboard'],
    queryFn: getDashboard,
  })
}

export function useLogin() {
  const setAuth = useAuthStore((s) => s.setAuth)
  const qc = useQueryClient()
  return useMutation({
    mutationFn: login,
    onSuccess: (data) => {
      setAuth({
        token: data.access_token,
        userId: data.user_id,
        email: data.email,
      })
      qc.invalidateQueries({ queryKey: ['me'] })
    },
  })
}

export function useRegister() {
  const setAuth = useAuthStore((s) => s.setAuth)
  const qc = useQueryClient()
  return useMutation({
    mutationFn: register,
    onSuccess: (data) => {
      setAuth({
        token: data.access_token,
        userId: data.user_id,
        email: data.email,
      })
      qc.invalidateQueries({ queryKey: ['me'] })
    },
  })
}

export function useCreateIssue() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: createIssue,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: projectKeys.all })
    },
  })
}

export function useMoveIssue(projectKey: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (input: MoveIssueInput) => moveIssue(projectKey, input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: projectKeys.detail(projectKey) })
      qc.invalidateQueries({ queryKey: ['backlog', projectKey] })
    },
  })
}


export function useUsers() {
  return useQuery({
    queryKey: ['users'],
    queryFn: listUsers,
    enabled: !!useAuthStore.getState().token,
  })
}

export function useCurrentUser() {
  return useQuery({
    queryKey: ['me'],
    queryFn: getCurrentUser,
    staleTime: 5 * 60 * 1000,
  })
}

export function useLogout() {
  const logoutStore = useAuthStore((s) => s.logout)
  const qc = useQueryClient()
  const navigate = useNavigate()
  return useMutation({
    mutationFn: logout,
    onSuccess: () => {
      logoutStore()
      qc.clear()
      navigate('/login')
    },
    onError: () => {
      logoutStore()
      navigate('/login')
    },
  })
}

export function useCreateProject() {
  const qc = useQueryClient()
  const navigate = useNavigate()
  return useMutation({
    mutationFn: createProject,
    onSuccess: (data) => {
      qc.invalidateQueries({ queryKey: projectKeys.all })
      navigate(`/projects/${data.key}/board`)
    },
  })
}

export function useUpdateProject(key: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: Parameters<typeof updateProject>[1]) => updateProject(key, input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: projectKeys.all })
      qc.invalidateQueries({ queryKey: projectKeys.detail(key) })
    },
  })
}

export function useDeleteProject() {
  const qc = useQueryClient()
  const navigate = useNavigate()
  return useMutation({
    mutationFn: deleteProject,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: projectKeys.all })
      navigate('/projects')
    },
  })
}

export function useUpdateIssue(id: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: Parameters<typeof updateIssue>[1]) => updateIssue(id, input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['issue', id] })
    },
  })
}

export function useDeleteIssue() {
  const qc = useQueryClient()
  const navigate = useNavigate()
  return useMutation({
    mutationFn: deleteIssue,
    onSuccess: (_data, id) => {
      qc.invalidateQueries({ queryKey: projectKeys.all })
      qc.removeQueries({ queryKey: ['issue', id] })
      navigate('/')
    },
  })
}

export function useProjectMembers(projectId: string) {
  return useQuery({
    queryKey: ['project-members', projectId],
    queryFn: () => listProjectMembers(projectId),
    enabled: !!projectId,
  })
}

export function useAddProjectMember(projectId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: AddProjectMemberInput) => addProjectMember(projectId, input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['project-members', projectId] })
    },
  })
}

export function useRemoveProjectMember(projectId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (userId: string) => removeProjectMember(projectId, userId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['project-members', projectId] })
    },
  })
}
