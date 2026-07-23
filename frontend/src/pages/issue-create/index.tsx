import { useState } from 'react'
import { Link } from 'react-router'
import { Plus } from 'lucide-react'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { createIssue } from '@/api/issue-create'

export function IssueCreatePage() {
  const [projectId, setProjectId] = useState('project-1')
  const [type, setType] = useState('Task')
  const [summary, setSummary] = useState('')
  const [description, setDescription] = useState('')
  const [priority, setPriority] = useState('Medium')
  const [assigneeId, setAssigneeId] = useState('')
  const [labels, setLabels] = useState('')
  const [dueDate, setDueDate] = useState('2026-08-15')

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    await createIssue({
      projectId,
      type: type as 'Task' | 'Story' | 'Bug' | 'Epic',
      summary,
      description,
      priority: priority as 'Highest' | 'High' | 'Medium' | 'Low' | 'Lowest',
      assigneeId: assigneeId || undefined,
      labels: labels.split(',').map((l) => l.trim()).filter(Boolean),
      dueDate: dueDate || null,
    })
    window.location.href = '/projects/project-1/backlog'
  }

  return (
    <div className="mx-auto max-w-3xl">
      <h1 className="mb-5 text-xl font-bold sm:text-2xl">Создать задачу</h1>

      <form
        onSubmit={handleSubmit}
        className="space-y-4 rounded-lg border border-border bg-surface p-4 sm:p-6"
      >
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">Проект *</label>
            <select
              className="h-10 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary"
              value={projectId}
              onChange={(e) => setProjectId(e.target.value)}
            >
              <option value="project-1">Task Tracker (TT)</option>
              <option value="project-2">Mobile App (MOB)</option>
              <option value="project-3">Public API (API)</option>
            </select>
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Тип задачи *</label>
            <select
              className="h-10 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary"
              value={type}
              onChange={(e) => setType(e.target.value)}
            >
              <option>Task</option>
              <option>Story</option>
              <option>Bug</option>
              <option>Epic</option>
            </select>
          </div>
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium">Название / Summary *</label>
          <Input
            type="text"
            placeholder="Краткое описание задачи"
            value={summary}
            onChange={(e) => setSummary(e.target.value)}
            required
          />
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium">Описание</label>
          <textarea
            className="min-h-[120px] w-full rounded-md border border-border-strong bg-background p-3 text-sm text-text-primary"
            placeholder="Подробное описание, acceptance criteria..."
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
        </div>

        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">Приоритет</label>
            <select
              className="h-10 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary"
              value={priority}
              onChange={(e) => setPriority(e.target.value)}
            >
              <option>Medium</option>
              <option>Highest</option>
              <option>High</option>
              <option>Low</option>
              <option>Lowest</option>
            </select>
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Исполнитель</label>
            <select
              className="h-10 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary"
              value={assigneeId}
              onChange={(e) => setAssigneeId(e.target.value)}
            >
              <option value="">Unassigned</option>
              <option value="user-2">Ivan</option>
              <option value="user-3">Anna</option>
              <option value="user-4">Petr</option>
            </select>
          </div>
        </div>

        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">Метки</label>
            <Input
              type="text"
              placeholder="backend, auth"
              value={labels}
              onChange={(e) => setLabels(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Срок выполнения</label>
            <Input
              type="date"
              value={dueDate}
              onChange={(e) => setDueDate(e.target.value)}
            />
          </div>
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium">Автор</label>
          <Input type="text" value="me (read-only)" disabled />
        </div>

        <div className="flex flex-wrap gap-2 pt-2">
          <Button type="submit" className="gap-1">
            <Plus className="h-4 w-4" />
            Создать
          </Button>
          <Button type="button" variant="outline">Создать ещё одну</Button>
          <Button variant="outline" asChild>
            <Link to="/projects/project-1/backlog">Отмена</Link>
          </Button>
        </div>
      </form>
    </div>
  )
}
