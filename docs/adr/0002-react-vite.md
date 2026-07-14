# ADR-0002: React 19.1.0 + Vite 6.2.0 for Frontend

## Status

Accepted

## Context

Нужен современный frontend-стек для сложного интерфейса Jira-like приложения: kanban, issue detail, dashboards, rich text.

## Alternatives Considered

| Option | Pros | Cons |
|--------|------|------|
| Vue 3 + Vite | Простота, хороший DX | Меньше готовых сложных компонентов для kanban/dnd |
| Next.js 15 | SSR, routing, API routes | Избыточно для self-hosted SPA |
| React 19.1.0 + Vite 6.2.0 | Гибкость, скорость, большая экосистема | Нужно настраивать router/state самому |

## Decision

React 19.1.0 + Vite 6.2.0 + TypeScript 5.9.3.

## Consequences

- Полный контроль над сборкой и роутингом.
- Большая экосистема компонентов (shadcn/ui, dnd-kit, tiptap).
- Отсутствие встроенного SSR — не критично для MVP.
- Нужно самостоятельно настроить PWA если потребуется.

## Related

- `docs/FRONTEND_ARCHITECTURE.md`
- `docs/UI_LIBRARIES.md`
