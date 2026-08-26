import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router'
import { listProjects, createProject, updateProject, deleteProject } from '@/api/project'
import { getBoard, getBacklog, moveIssue, type MoveIssueInput } from '@/api/board'
import { searchIssues, type SearchFilters } from '@/api/search'
import { login, register, getCurrentUser, listUsers, logout } from '@/api/auth'
import { createIssue } from '@/api/issue-create'
import {
  updateIssue,
  deleteIssue,
  restoreIssue,
  purgeIssue,
  listTrash,
  getIssue,
} from '@/api/issue'
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
import { listAttachments, uploadAttachment, deleteAttachment } from '@/api/attachment'
import {
  listProjectLabels,
  createLabel,
  listIssueLabels,
  attachLabel,
  detachLabel,
} from '@/api/label'
import { listIssueLinks, createIssueLink, deleteIssueLink } from '@/api/link'
import {
  createCustomField,
  deleteCustomField,
  listCustomFields,
  listIssueCustomFieldValues,
  setIssueCustomFieldValue,
  type CustomFieldInput,
} from '@/api/custom-fields'
import {
  getVelocityReport,
  getBurndownReport,
  getCumulativeFlowReport,
  getControlChartReport,
} from '@/api/reports'
import {
  getNotificationSettings,
  listNotifications,
  markAllNotificationsRead,
  markNotificationRead,
  updateNotificationSettings,
  type UpdateNotificationSettingsInput,
} from '@/api/notifications'
import {
  createAdminUser,
  listAdminAuditLog,
  listAdminSettings,
  listAdminUsers,
  updateAdminSetting,
  updateAdminUserStatus,
  type CreateAdminUserInput,
  type UpdateSystemSettingInput,
} from '@/api/admin'

const adminKeys = {
  all: ['admin'] as const,
  users: ['admin', 'users'] as const,
  settings: ['admin', 'settings'] as const,
  auditLog: (limit?: number) => ['admin', 'audit-log', limit ?? 100] as const,
}

export function useAdminUsers() {
  return useQuery({ queryKey: adminKeys.users, queryFn: listAdminUsers })
}

export function useCreateAdminUser() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: CreateAdminUserInput) => createAdminUser(input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: adminKeys.users })
      qc.invalidateQueries({ queryKey: adminKeys.auditLog() })
    },
  })
}

export function useUpdateAdminUserStatus() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: updateAdminUserStatus,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: adminKeys.users })
      qc.invalidateQueries({ queryKey: adminKeys.auditLog() })
    },
  })
}

export function useAdminSettings() {
  return useQuery({ queryKey: adminKeys.settings, queryFn: listAdminSettings })
}

export function useUpdateAdminSetting() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: UpdateSystemSettingInput) => updateAdminSetting(input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: adminKeys.settings })
      qc.invalidateQueries({ queryKey: adminKeys.auditLog() })
    },
  })
}

export function useAdminAuditLog(limit = 100) {
  return useQuery({
    queryKey: adminKeys.auditLog(limit),
    queryFn: () => listAdminAuditLog(limit),
  })
}

const notificationKeys = {
  list: ['notifications'] as const,
  settings: ['notification-settings'] as const,
}

export function useNotifications() {
  return useQuery({ queryKey: notificationKeys.list, queryFn: listNotifications })
}

export function useMarkNotificationRead() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: markNotificationRead,
    onSuccess: () => qc.invalidateQueries({ queryKey: notificationKeys.list }),
  })
}

export function useMarkAllNotificationsRead() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: markAllNotificationsRead,
    onSuccess: () => qc.invalidateQueries({ queryKey: notificationKeys.list }),
  })
}

export function useNotificationSettings() {
  return useQuery({ queryKey: notificationKeys.settings, queryFn: getNotificationSettings })
}

export function useUpdateNotificationSettings() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: UpdateNotificationSettingsInput) => updateNotificationSettings(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: notificationKeys.settings }),
  })
}

const workflowKeys = {
  statuses: ['statuses'] as const,
  transitions: ['transitions'] as const,
  issueTypes: ['issue-types'] as const,
}

export function useStatuses() {
  return useQuery({
    queryKey: workflowKeys.statuses,
    queryFn: listStatuses,
    staleTime: 5 * 60 * 1000,
  })
}

export function useTransitions() {
  return useQuery({
    queryKey: workflowKeys.transitions,
    queryFn: listTransitions,
    staleTime: 5 * 60 * 1000,
  })
}

export function useIssueTypes() {
  return useQuery({
    queryKey: workflowKeys.issueTypes,
    queryFn: listIssueTypes,
    staleTime: 5 * 60 * 1000,
  })
}

const attachmentKeys = {
  list: (issueId: string) => ['attachments', issueId] as const,
}

export function useAttachments(issueId: string | undefined) {
  return useQuery({
    queryKey: attachmentKeys.list(issueId ?? ''),
    queryFn: () => listAttachments(issueId!),
    enabled: !!issueId,
  })
}

export function useUploadAttachment(issueId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({
      file,
      onProgress,
    }: {
      file: File
      onProgress?: (loaded: number, total: number) => void
    }) => uploadAttachment(issueId, file, onProgress),
    onSettled: () => qc.invalidateQueries({ queryKey: attachmentKeys.list(issueId) }),
  })
}

export function useDeleteAttachment(issueId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => deleteAttachment(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: attachmentKeys.list(issueId) }),
  })
}

const projectKeys = {
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

export function useIssue(id: string) {
  return useQuery({
    queryKey: ['issue', id],
    queryFn: () => getIssue(id),
    refetchOnWindowFocus: false,
    staleTime: 0,
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

export function useTrash(projectKey: string | undefined) {
  return useQuery({
    queryKey: ['trash', projectKey],
    queryFn: () => listTrash(projectKey!),
    enabled: !!projectKey,
  })
}

export function useRestoreIssue() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: restoreIssue,
    onSuccess: (data) => {
      qc.invalidateQueries({ queryKey: ['trash'] })
      qc.invalidateQueries({ queryKey: projectKeys.all })
      qc.setQueryData(['issue', data.id], data)
    },
  })
}

export function usePurgeIssue() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: purgeIssue,
    onSuccess: (_data, id) => {
      qc.invalidateQueries({ queryKey: ['trash'] })
      qc.removeQueries({ queryKey: ['issue', id] })
    },
  })
}

export function useProjectMembers(projectKey: string) {
  return useQuery({
    queryKey: ['project-members', projectKey],
    queryFn: () => listProjectMembers(projectKey),
    enabled: !!projectKey,
  })
}

export function useAddProjectMember(projectKey: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: AddProjectMemberInput) => addProjectMember(projectKey, input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['project-members', projectKey] })
    },
  })
}

export function useRemoveProjectMember(projectKey: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (userId: string) => removeProjectMember(projectKey, userId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['project-members', projectKey] })
    },
  })
}

const labelKeys = {
  project: (projectKey: string) => ['labels', projectKey] as const,
  issue: (issueId: string) => ['issue-labels', issueId] as const,
}

export function useProjectLabels(projectKey: string | undefined) {
  return useQuery({
    queryKey: labelKeys.project(projectKey ?? ''),
    queryFn: () => listProjectLabels(projectKey!),
    enabled: !!projectKey,
  })
}

export function useCreateLabel(projectKey: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ name, color }: { name: string; color: string }) =>
      createLabel(projectKey, name, color),
    onSuccess: () => qc.invalidateQueries({ queryKey: labelKeys.project(projectKey) }),
  })
}

export function useIssueLabels(issueId: string | undefined) {
  return useQuery({
    queryKey: labelKeys.issue(issueId ?? ''),
    queryFn: () => listIssueLabels(issueId!),
    enabled: !!issueId,
  })
}

export function useAttachLabel(issueId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (labelId: string) => attachLabel(issueId, labelId),
    onSuccess: () => qc.invalidateQueries({ queryKey: labelKeys.issue(issueId) }),
  })
}

export function useDetachLabel(issueId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (labelId: string) => detachLabel(issueId, labelId),
    onSuccess: () => qc.invalidateQueries({ queryKey: labelKeys.issue(issueId) }),
  })
}

const linkKeys = {
  issue: (issueId: string) => ['issue-links', issueId] as const,
}

export function useIssueLinks(issueId: string | undefined) {
  return useQuery({
    queryKey: linkKeys.issue(issueId ?? ''),
    queryFn: () => listIssueLinks(issueId!),
    enabled: !!issueId,
  })
}

export function useCreateIssueLink(issueId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ targetKey, linkType }: { targetKey: string; linkType: string }) =>
      createIssueLink(issueId, targetKey, linkType),
    onSuccess: () => qc.invalidateQueries({ queryKey: linkKeys.issue(issueId) }),
  })
}

export function useDeleteIssueLink(issueId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => deleteIssueLink(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: linkKeys.issue(issueId) }),
  })
}

const reportKeys = {
  velocity: (projectId: string, count: number) =>
    ['reports', 'velocity', projectId, count] as const,
  burndown: (sprintId: string) => ['reports', 'burndown', sprintId] as const,
  cumulativeFlow: (projectId: string) => ['reports', 'cumulative-flow', projectId] as const,
  controlChart: (projectId: string) => ['reports', 'control-chart', projectId] as const,
}

export function useVelocityReport(projectId: string | undefined, count = 6) {
  return useQuery({
    queryKey: reportKeys.velocity(projectId ?? '', count),
    queryFn: () => getVelocityReport(projectId!, count),
    enabled: !!projectId,
  })
}

export function useBurndownReport(sprintId: string | undefined) {
  return useQuery({
    queryKey: reportKeys.burndown(sprintId ?? ''),
    queryFn: () => getBurndownReport(sprintId!),
    enabled: !!sprintId,
  })
}

export function useCumulativeFlowReport(projectId: string | undefined) {
  return useQuery({
    queryKey: reportKeys.cumulativeFlow(projectId ?? ''),
    queryFn: () => getCumulativeFlowReport(projectId!),
    enabled: !!projectId,
  })
}

export function useControlChartReport(projectId: string | undefined) {
  return useQuery({
    queryKey: reportKeys.controlChart(projectId ?? ''),
    queryFn: () => getControlChartReport(projectId!),
    enabled: !!projectId,
  })
}

const customFieldKeys = {
  project: (projectKey: string) => ['custom-fields', projectKey] as const,
  issue: (issueId: string) => ['issue-custom-fields', issueId] as const,
}

export function useProjectCustomFields(projectKey: string | undefined) {
  return useQuery({
    queryKey: customFieldKeys.project(projectKey ?? ''),
    queryFn: () => listCustomFields(projectKey!),
    enabled: !!projectKey,
  })
}

export function useCreateCustomField(projectKey: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: CustomFieldInput) => createCustomField(projectKey, input),
    onSuccess: () => qc.invalidateQueries({ queryKey: customFieldKeys.project(projectKey) }),
  })
}

export function useDeleteCustomField(projectKey: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: deleteCustomField,
    onSuccess: () => qc.invalidateQueries({ queryKey: customFieldKeys.project(projectKey) }),
  })
}

export function useIssueCustomFieldValues(issueId: string | undefined) {
  return useQuery({
    queryKey: customFieldKeys.issue(issueId ?? ''),
    queryFn: () => listIssueCustomFieldValues(issueId!),
    enabled: !!issueId,
  })
}

export function useSetIssueCustomFieldValue(issueId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ fieldId, value }: { fieldId: string; value: unknown }) =>
      setIssueCustomFieldValue(issueId, fieldId, value),
    onSuccess: () => qc.invalidateQueries({ queryKey: customFieldKeys.issue(issueId) }),
  })
}
