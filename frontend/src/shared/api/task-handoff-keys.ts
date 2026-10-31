export const handoffKeys = {
  all: ['task-handoff'] as const,
  task: (id: string) => ['task-handoff', id] as const,
  view: (id: string, scope: string) => ['task-handoff', id, scope] as const,
}
