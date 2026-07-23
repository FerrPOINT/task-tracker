import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { listProjects } from '@/api/project'
import { getBoard, getBacklog } from '@/api/board'
import { searchIssues } from '@/api/search'
import { login, register } from '@/api/auth'
import { createIssue } from '@/api/issue-create'
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
  const setToken = useAuthStore((s) => s.setToken)
  return useMutation({
    mutationFn: login,
    onSuccess: (data) => {
      setToken(data.access_token)
    },
  })
}

export function useRegister() {
  const setToken = useAuthStore((s) => s.setToken)
  return useMutation({
    mutationFn: register,
    onSuccess: (data) => {
      setToken(data.access_token)
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
