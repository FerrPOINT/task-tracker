import { useState } from 'react'
import { Link } from 'react-router'
import { Plus } from 'lucide-react'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { createIssue } from '@/api/issue-create'

export function IssueCreatePage() {
  const [project_key, setProjectKey] = useState('TT')
  const [type, setType] = useState('Task')
  const [summary, setSummary] = useState('')
  const [description, setDescription] = useState('')
  const [priority, setPriority] = useState('Medium')
  const [assignee_id, setAssigneeId] = useState('')
  const [labels, setLabels] = useState('')
  const [due_date, setDueDate] = useState('2026-08-15')
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setSubmitting(true)
    setError(null)
    try {
      await createIssue({
        project_key,
        issue_type: type as 'Task' | 'Story' | 'Bug' | 'Epic',
        summary,
        description,
        priority: priority as 'Highest' | 'High' | 'Medium' | 'Low' | 'Lowest',
        assignee_id: assignee_id || null,
        labels: labels.split(',').map((l) => l.trim()).filter(Boolean),
        due_date: due_date || null,
      })
      window.location.href = `/projects/${project_key}/backlog`
    } catch (e) {
      setError(e instanceof Error ? e.message : 'failed to create issue')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="mx-auto max-w-3xl">
      <h1 className="mb-5 text-xl font-bold sm:text-2xl">Создать задачу</h1>

      <form
        onSubmit={handleSubmit}
        className="space-y-4 rounded-lg border border-border bg-surface p-4 sm:p-6"
      >
        {error && <div className="text-sm text-rose-500">{error}</div>}

        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <label className="text-sm font-medium">Проект *</label>
            <select
              className="h-10 w-full rounded-md border border-border-strong bg-background px-3 text-sm text-text-primary"
              value={project_key}
              onChange={(e) => setProjectKey(e.target.value)}
            >
              <option value="TT">Task Tracker (TT)</option>
              <option value="MOB">Mobile App (MOB)</option>
              <option value="API">Public API (API)</option>
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
              value={assignee_id}
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
              value={due_date}
              onChange={(e) => setDueDate(e.target.value)}
            />
          </div>
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium">Автор</label>
          <Input type="text" value="me (read-only)" disabled />
        </div>

        <div className="flex flex-wrap gap-2 pt-2">
          <Button type="submit" disabled={submitting} className="gap-1">
            <Plus className="h-4 w-4" />
            {submitting ? 'Создание…' : 'Создать'}
          </Button>
          <Button type="button" variant="outline">Создать ещё одну</Button>
          <Button variant="outline" asChild>
            <Link to={`/projects/${project_key}/backlog`}>Отмена</Link>
          </Button>
        </div>
      </form>
    </div>
  )
}
