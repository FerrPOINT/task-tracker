import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router'
import { listProjects } from '@/api/project'
import { getBoard, getBacklog, moveIssue, type MoveIssueInput } from '@/api/board'
import { searchIssues } from '@/api/search'
import { login, register, getCurrentUser, listUsers } from '@/api/auth'
import { createIssue } from '@/api/issue-create'
import { updateIssue, deleteIssue } from '@/api/issue'
import { getDashboard } from '@/api/dashboard'
import { useAuthStore } from '@/shared/auth/store'

export const projectKeys = {
  all: ['projects'] as const,
  detail: (key: string) => ['project', key] as const,
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

export function useSearch(q: string) {
  return useQuery({
    queryKey: ['search', q],
    queryFn: () => searchIssues(q),
  })
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
