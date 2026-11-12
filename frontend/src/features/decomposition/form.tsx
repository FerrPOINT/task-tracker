import { useRef, type FormEvent } from 'react'
import {
  Card,
  CardHeader,
  SectionHeader,
  CardContent,
  Fieldset,
  ActionGroup,
  Notice,
  Button,
} from '@pdlc/ui/ui'
import { ArrowUp, ArrowDown, Plus, Trash2 } from 'lucide-react'
import { Field, type ReadablePlan, type Section } from './plan'
export function PlanForm<T extends ReadablePlan>({
  draft,
  setDraft,
  reason,
  setReason,
  pending,
  error,
  reasonRequired,
  blocked = false,
  savePlan,
  cancelEdit,
  createId = () => crypto.randomUUID(),
}: {
  draft: T
  setDraft: (value: T) => void
  reason: string
  setReason: (value: string) => void
  pending: boolean
  error: string | null
  reasonRequired: boolean
  blocked?: boolean
  savePlan: (event: FormEvent) => void
  cancelEdit: () => void
  createId?: () => string
}) {
  const addSectionRef = useRef<HTMLButtonElement>(null)
  function updateSection(id: string, patch: Partial<Section>) {
    setDraft({
      ...draft,
      sections: draft.sections.map((s) => (s.id === id ? { ...s, ...patch } : s)),
    })
  }
  function move(id: string, direction: number) {
    const sections = [...draft.sections]
    const index = sections.findIndex((s) => s.id === id)
    const next = index + direction
    if (index < 0 || next < 0 || next >= sections.length) return
    ;[sections[index], sections[next]] = [sections[next]!, sections[index]!]
    setDraft({ ...draft, sections })
  }
  return (
    <form
      aria-label="Редактирование декомпозиции"
      onSubmit={savePlan}
      className="grid max-w-4xl gap-4"
      aria-busy={pending}
    >
      <Fieldset disabled={pending} className="space-y-4">
        <Field
          id="plan-overview"
          label="Архитектурное решение"
          multiline
          required
          value={draft.overview}
          onChange={(overview) => setDraft({ ...draft, overview })}
        />
        <div className="space-y-3">
          {draft.sections.map((section, index) => (
            <Card key={section.id} data-testid={`editor-${section.id}`}>
              <CardHeader>
                <SectionHeader
                  title={`Раздел ${index + 1}`}
                  level={3}
                  actions={
                    <>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        aria-label={`Переместить ${section.title || 'раздел'} вверх`}
                        disabled={index === 0}
                        onClick={() => move(section.id, -1)}
                      >
                        <ArrowUp />
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        aria-label={`Переместить ${section.title || 'раздел'} вниз`}
                        disabled={index === draft.sections.length - 1}
                        onClick={() => move(section.id, 1)}
                      >
                        <ArrowDown />
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        onClick={() => {
                          setDraft({
                            ...draft,
                            sections: draft.sections.filter((s) => s.id !== section.id),
                          })
                          const neighbor = draft.sections[index + 1] ?? draft.sections[index - 1]
                          queueMicrotask(() => {
                            if (neighbor) document.getElementById(`${neighbor.id}-title`)?.focus()
                            else addSectionRef.current?.focus()
                          })
                        }}
                      >
                        <Trash2 />
                        Убрать раздел
                      </Button>
                    </>
                  }
                />
              </CardHeader>
              <CardContent className="space-y-4">
                <Field
                  id={`${section.id}-title`}
                  label="Название раздела"
                  required
                  value={section.title}
                  onChange={(title) => updateSection(section.id, { title })}
                />
                <Field
                  id={`${section.id}-description`}
                  label="Что сделать"
                  hint="Опишите изменение и ожидаемый результат."
                  multiline
                  required
                  value={section.description}
                  onChange={(description) => updateSection(section.id, { description })}
                />
                <Field
                  id={`${section.id}-criteria`}
                  label="Как проверить"
                  multiline
                  required
                  value={section.criteria}
                  onChange={(criteria) => updateSection(section.id, { criteria })}
                />
                <Field
                  id={`${section.id}-requirements`}
                  label="Связь с требованиями"
                  required
                  value={section.requirements}
                  onChange={(requirements) => updateSection(section.id, { requirements })}
                />
              </CardContent>
            </Card>
          ))}
        </div>
        <Button
          ref={addSectionRef}
          type="button"
          variant="outline"
          onClick={() => {
            const newId = createId()
            setDraft({
              ...draft,
              sections: [
                ...draft.sections,
                {
                  id: newId,
                  title: '',
                  description: '',
                  criteria: '',
                  requirements: '',
                },
              ],
            })
            queueMicrotask(() => document.getElementById(`${newId}-title`)?.focus())
          }}
        >
          <Plus />
          Добавить раздел
        </Button>
        <Field
          id="plan-reason"
          label="Причина изменения"
          required={reasonRequired}
          value={reason}
          onChange={setReason}
          hint="Сохранится вместе с новой редакцией плана."
        />
        <Notice appearance="inline">
          Содержательно изменённые разделы потребуют повторной проверки. Переименование и порядок
          сохраняют ссылки.
        </Notice>
        {error && (
          <Notice role="alert" tone="danger">
            {error}
          </Notice>
        )}
        <ActionGroup separated>
          <Button type="submit" disabled={pending || blocked}>
            {pending ? 'Сохраняем…' : 'Сохранить план'}
          </Button>
          <Button type="button" variant="outline" disabled={pending} onClick={cancelEdit}>
            Отмена
          </Button>
        </ActionGroup>
      </Fieldset>
    </form>
  )
}
