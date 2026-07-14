# Architecture Decision Records — Task Tracker

## 1. Overview

ADR фиксируют ключевые архитектурные решения: контекст, альтернативы, выбор, последствия. Хранятся в `docs/adr/`.

## 2. Format

Каждый ADR — файл `NNNN-title.md`:

```markdown
# ADR-0001: Title

## Status

Proposed / Accepted / Deprecated / Superseded by ADR-NNNN

## Context

What problem are we solving?

## Decision

What did we decide?

## Consequences

Positive and negative.
```

## 3. Active ADRs

| ID | Title | Status |
|----|-------|--------|
| ADR-0001 | Rust + Axum for backend | Accepted |
| ADR-0002 | React 19.1.0 + Vite 6.2.0 for frontend | Accepted |
| ADR-0003 | PostgreSQL as primary database | Accepted |
| ADR-0004 | SeaORM + SQLx for DB access | Accepted |
| ADR-0005 | Feature-Sliced Design for frontend | Accepted |
| ADR-0006 | JWT access + httpOnly refresh cookie | Accepted |
| ADR-0007 | Redis for cache and WS pub/sub | Accepted |
| ADR-0008 | shadcn/ui + Tailwind CSS 4.1.0 | Accepted |
| ADR-0009 | TanStack Query + Zustand for state | Accepted |
| ADR-0010 | apalis for background jobs | Accepted |

## 4. Creating New ADRs

1. Взять следующий номер.
2. Создать `docs/adr/NNNN-title.md`.
3. Обновить index в этом файле.
4. Открыть PR.

## 5. Superseding

Если решение меняется:

1. Новый ADR со статусом `Accepted`.
2. Старый ADR меняет статус на `Superseded by ADR-NNNN`.

## 6. Principles

- One significant decision — one ADR.
- Keep ADRs concise (1-2 pages).
- Link to related docs.
- Russian or English? English for consistency with code/docs codebase.
## References

- `docs/ARCHITECTURE.md`
- `CONTRIBUTING.md` (корень репозитория)
