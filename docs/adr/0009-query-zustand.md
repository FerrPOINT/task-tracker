# ADR-0009: TanStack Query + Zustand for State

## Status

Accepted

## Context

Нужно управление server state и client state в React-приложении.

## Alternatives Considered

| Option | Pros | Cons |
|--------|------|------|
| Redux Toolkit | Зрелый, predictable | Много boilerplate |
| MobX | Простой reactive | Магия, сложно debug |
| TanStack Query | Server state кэширование | Только server state |
| Zustand | Минималистичный | Только client state |
| Query + Zustand | Разделение ответственности | Два инструмента |

## Decision

TanStack Query для server state, Zustand для client UI state.

## Consequences

- Автоматическое кеширование, инвалидация, retry.
- Минимальный client state store.
- Чёткое разделение: серверное vs UI-состояние.

## Related

- `docs/FRONTEND_ARCHITECTURE.md`
- `docs/CACHING.md`
