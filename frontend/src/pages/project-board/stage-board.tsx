import { useState, type DragEvent } from 'react'
import { useParams, useSearchParams } from 'react-router'
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  ErrorState,
  LoadingState,
  PageHeader,
  ProjectAvatar,
  RadioGroup,
  Select,
  TextLink,
  UserAvatar,
  toast,
} from '@pdlc/ui/ui'
import { ProjectLink as Link } from '@/shared/lib/project-navigation'
import { useLifecycleTransition, useStageBoard } from '@/features/issues/model/task-lifecycle'
import { TaskLifecycleActions } from '@/features/issues/ui/TaskLifecyclePanel'
import { lifecycleStatusLabels, stageLabels } from '@/shared/lib/task-lifecycle-labels'
import { CreateTaskAction } from '@/features/issues/ui/CreateTaskAction'
import type { StageBoard, TaskStage } from '@/api/task-lifecycle'

type CardData = StageBoard['issues'][number]
export function TaskStageBoard() {
  const { projectKey: key = '' } = useParams()
  const [search, setSearch] = useSearchParams()
  const query = useStageBoard(key)
  const change = useLifecycleTransition()
  const [dragged, setDragged] = useState<string | null>(null)
  const [dropStage, setDropStage] = useState<TaskStage | null>(null)
  const density = search.get('density') === 'compact' ? 'compact' : 'default'
  if (query.isPending) return <LoadingState message="Загружаем задачи…" />
  if (!query.data) return <ErrorState message={query.error?.message ?? 'Доска недоступна'} />
  const board = query.data
  const cardsById = new Map(board.issues.map((item) => [item.issue.id, item]))
  const selected =
    board.columns.find((column) => column.stage === search.get('stage'))?.stage ??
    board.columns[0]?.stage
  function drop(event: DragEvent, stage: TaskStage) {
    event.preventDefault()
    const id = event.dataTransfer.getData('text/plain') || dragged
    const card = id ? cardsById.get(id) : undefined
    const option = card?.lifecycle.allowed_actions.find(
      (action) =>
        action.target.stage === stage && action.target.stage !== card.lifecycle.state.stage,
    )
    if (card && option)
      change.mutate(
        {
          id: card.issue.id,
          command: {
            action: option.action,
            expected_revision: card.lifecycle.state.revision,
            idempotency_key: crypto.randomUUID(),
          },
        },
        { onError: (error) => toast.error(error.message) },
      )
    setDragged(null)
    setDropStage(null)
  }
  return (
    <div className="min-w-0 space-y-4">
      <PageHeader
        title={`Задачи · ${key}`}
        leading={<ProjectAvatar projectKey={key} />}
        actions={
          <>
            <Button asChild variant="outline">
              <Link to={`/projects/${key}/backlog`}>Бэклог</Link>
            </Button>
            <CreateTaskAction projectKey={key} />
          </>
        }
      />
      <RadioGroup
        appearance="segmented"
        label="Плотность карточек"
        value={density}
        options={[
          { value: 'default', label: 'Обычная' },
          { value: 'compact', label: 'Компактная' },
        ]}
        onValueChange={(value) => {
          const next = new URLSearchParams(search)
          next.set('density', value)
          setSearch(next, { replace: true })
        }}
      />
      <div className="md:hidden">
        <Select
          aria-label="Стадия задач"
          value={selected}
          options={board.columns.map((column) => ({
            value: column.stage,
            label: stageLabels[column.stage],
          }))}
          onValueChange={(stage) => {
            const next = new URLSearchParams(search)
            next.set('stage', stage)
            setSearch(next, { replace: true })
          }}
        />
      </div>
      <div
        role="region"
        aria-label="Стадии задач"
        tabIndex={0}
        className="grid min-w-0 grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3"
      >
        {board.columns.map((column) => {
          const cards = column.issue_ids
            .map((id) => cardsById.get(id))
            .filter((item): item is CardData => !!item)
          const canDrop = board.issues.some(
            (card) =>
              card.issue.id === dragged &&
              card.lifecycle.allowed_actions.some(
                (option) =>
                  option.target.stage === column.stage &&
                  option.target.stage !== card.lifecycle.state.stage,
              ),
          )
          return (
            <Card
              key={column.stage}
              highlighted={dropStage === column.stage}
              className={`${column.stage === selected ? 'flex' : 'hidden'} min-w-0 flex-col md:flex`}
              onDragOver={(event) => {
                if (canDrop) {
                  event.preventDefault()
                  setDropStage(column.stage)
                }
              }}
              onDragLeave={() => setDropStage(null)}
              onDrop={(event) => drop(event, column.stage)}
            >
              <CardHeader density="compact">
                <div className="flex items-center justify-between gap-2">
                  <h2 className="text-sm font-semibold">{stageLabels[column.stage]}</h2>
                  <Badge
                    tone="muted"
                    aria-label={
                      column.wip_limit == null
                        ? `${cards.length} задач`
                        : `${cards.length} задач, WIP ${column.wip_limit}`
                    }
                  >
                    {cards.length}
                    {column.wip_limit == null ? null : ` / ${column.wip_limit}`}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent density="compact" className="min-h-24 space-y-2">
                {cards.map(({ issue, lifecycle }) => (
                  <Card key={issue.id} surface="raised" asChild>
                    <article
                      draggable={
                        !change.isPending &&
                        lifecycle.allowed_actions.some(
                          (option) => option.target.stage !== lifecycle.state.stage,
                        )
                      }
                      onDragStart={(event) => {
                        setDragged(issue.id)
                        event.dataTransfer.setData('text/plain', issue.id)
                      }}
                      onDragEnd={() => {
                        setDragged(null)
                        setDropStage(null)
                      }}
                    >
                      <CardContent density={density} className="space-y-2">
                        <TextLink asChild>
                          <Link to={`/issues/${issue.id}`}>
                            <span className="text-xs text-text-muted">{issue.key}</span>
                            <div className="break-words text-sm font-medium">{issue.summary}</div>
                          </Link>
                        </TextLink>
                        <div className="flex flex-wrap items-center gap-2">
                          <Badge tone="muted">
                            {lifecycleStatusLabels[lifecycle.state.status]}
                          </Badge>
                          {issue.assignee_name && (
                            <span className="flex items-center gap-1 text-xs">
                              <UserAvatar
                                name={issue.assignee_name}
                                userId={issue.assignee_id ?? undefined}
                                size="sm"
                              />
                              {issue.assignee_name}
                            </span>
                          )}
                        </div>
                        {lifecycle.execution_busy && (
                          <p className="text-xs text-text-muted">
                            Агентное выполнение ещё не завершено
                          </p>
                        )}
                        <TaskLifecycleActions lifecycle={lifecycle} />
                      </CardContent>
                    </article>
                  </Card>
                ))}
              </CardContent>
            </Card>
          )
        })}
      </div>
    </div>
  )
}
