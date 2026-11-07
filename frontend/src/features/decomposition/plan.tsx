import { useEffect, useState } from 'react'
import { Link, useLocation } from 'react-router'
import {
  Button,
  Disclosure,
  FormField,
  Input,
  Textarea,
  Prose,
  SectionHeader,
  Notice,
} from '@pdlc/ui/ui'
import { Link as LinkIcon } from 'lucide-react'
export interface Section {
  id: string
  title: string
  description: string
  criteria: string
  requirements: string
}
export interface ReadablePlan {
  overview: string
  sections: Section[]
}
export function Field({
  id,
  label,
  value,
  onChange,
  multiline = false,
  required = false,
  hint,
}: {
  id: string
  label: string
  value: string
  onChange: (value: string) => void
  multiline?: boolean
  required?: boolean
  hint?: string
}) {
  return (
    <FormField id={id} label={label} required={required} hint={hint}>
      {(attrs) =>
        multiline ? (
          <Textarea
            {...attrs}
            rows={3}
            value={value}
            onChange={(event) => onChange(event.target.value)}
          />
        ) : (
          <Input {...attrs} value={value} onChange={(event) => onChange(event.target.value)} />
        )
      }
    </FormField>
  )
}

export function PlanRead({ plan, snapshot = false }: { plan: ReadablePlan; snapshot?: boolean }) {
  const location = useLocation()
  const sectionSearch = new URLSearchParams(location.search)
  sectionSearch.set('tab', 'decomposition')
  const [open, setOpen] = useState<Set<string>>(() => new Set())
  useEffect(() => {
    const id = location.hash.replace('#section-', '')
    if (snapshot || !id || !plan.sections.some((s) => s.id === id)) return
    setOpen((value) => new Set([...value, id]))
    requestAnimationFrame(() => {
      const element = document.getElementById(`section-${id}`)
      element?.scrollIntoView({ block: 'nearest' })
      element?.querySelector('summary')?.focus()
    })
  }, [location.hash, plan.sections, snapshot])
  return (
    <div className="min-w-0 space-y-2 break-words">
      {!snapshot &&
        location.hash.startsWith('#section-') &&
        !plan.sections.some((s) => `#section-${s.id}` === location.hash) && (
          <Notice tone="warning">
            Раздела нет в текущей редакции. Предыдущие редакции доступны по номеру плана.
          </Notice>
        )}
      <Disclosure
        appearance="row"
        summary="Архитектурное решение"
        data-testid="architecture-overview"
      >
        <Prose preserveLineBreaks>{plan.overview}</Prose>
      </Disclosure>
      <div>
        {plan.sections.map((section, index) => (
          <div key={section.id} className="relative">
            <Disclosure
              appearance="row"
              id={`${snapshot ? 'snapshot-' : ''}section-${section.id}`}
              open={open.has(section.id)}
              onToggle={(event) => {
                const expanded = event.currentTarget.open
                setOpen((value) => {
                  if (value.has(section.id) === expanded) return value
                  const next = new Set(value)
                  if (expanded) next.add(section.id)
                  else next.delete(section.id)
                  return next
                })
              }}
              summary={
                <span className={snapshot ? undefined : 'block pr-12'}>
                  {index + 1}. {section.title}
                </span>
              }
            >
              <div className="space-y-4 py-2">
                <SectionHeader title="Что сделать" level={4} />
                <Prose preserveLineBreaks>{section.description}</Prose>
                <SectionHeader title="Как проверить" level={4} />
                <Prose preserveLineBreaks>{section.criteria}</Prose>
                <SectionHeader title="Связь с требованиями" level={4} />
                <Prose preserveLineBreaks>{section.requirements}</Prose>
              </div>
            </Disclosure>
            {!snapshot && (
              <Button asChild variant="ghost" size="icon" className="absolute right-0 top-0">
                <Link
                  to={{ search: sectionSearch.toString(), hash: `section-${section.id}` }}
                  aria-label={`Ссылка на раздел «${section.title}»`}
                  title="Ссылка на раздел"
                >
                  <LinkIcon className="h-4 w-4" aria-hidden />
                </Link>
              </Button>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}
