# ADR-0008: shadcn/ui + Tailwind CSS v4

## Status

Accepted

## Context

Нужна современная, консистентная, кастомизируемая UI-система для React-приложения.

## Alternatives Considered

| Option | Pros | Cons |
|--------|------|------|
| Material UI | Готовый дизайн | Трудно кастомизировать под Jira-like |
| Ant Design | Богатые компоненты | Тяжеловат, less modern |
| Chakra UI | Хороший DX | v3 beta, менее стабильно |
| shadcn/ui + Tailwind | Copy-paste компоненты, полный контроль | Нужно самому собирать library |

## Decision

shadcn/ui компоненты на Radix primitives + Tailwind CSS v4 + CSS variables.

## Consequences

- Полный контроль над внешним видом.
- Design tokens через CSS variables.
- Не зависим от UI library vendor.
- Требует внимательного обновления shadcn-компонентов.

## Related

- `docs/DESIGN_TOKENS.md`
- `docs/REACT_STYLING.md`
- `docs/UI_LIBRARIES.md`
