export interface Project {
  id: string
  key: string
  name: string
  description: string
  type: 'Scrum' | 'Kanban' | 'Basic'
  leadId: string
  leadName: string
  totalIssues: number
  todo: number
  inProgress: number
  done: number
  isFavorite: boolean
}

export const projects: Project[] = [
  {
    id: 'project-1',
    key: 'TT',
    name: 'Task Tracker',
    description: 'Self-hosted issue tracker for small teams.',
    type: 'Scrum',
    leadId: 'user-2',
    leadName: 'Ivan',
    totalIssues: 42,
    todo: 12,
    inProgress: 5,
    done: 18,
    isFavorite: true,
  },
  {
    id: 'project-2',
    key: 'MOB',
    name: 'Mobile App',
    description: 'Companion mobile client for iOS and Android.',
    type: 'Kanban',
    leadId: 'user-3',
    leadName: 'Anna',
    totalIssues: 128,
    todo: 36,
    inProgress: 5,
    done: 87,
    isFavorite: false,
  },
  {
    id: 'project-3',
    key: 'API',
    name: 'Public API',
    description: 'Public REST and GraphQL APIs.',
    type: 'Basic',
    leadId: 'user-4',
    leadName: 'Petr',
    totalIssues: 15,
    todo: 7,
    inProgress: 0,
    done: 4,
    isFavorite: false,
  },
]

export async function listProjects(): Promise<Project[]> {
  await new Promise((resolve) => setTimeout(resolve, 300))
  return [...projects]
}

export async function getProject(id: string): Promise<Project | null> {
  await new Promise((resolve) => setTimeout(resolve, 200))
  return projects.find((p) => p.id === id) ?? null
}
