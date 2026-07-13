# ADR-0005: Feature-Sliced Design for Frontend

## Status

Accepted

## Context

Нужна масштабируемая frontend-архитектура для Jira-like приложения с большим количеством фич и экранов.

## Alternatives Considered

| Option | Pros | Cons |
|--------|------|------|
| By type (components/hooks) | Просто | Быстро превращается в мусор |
| Atomic Design | Хорошо для UI | Плохо для бизнес-логики |
| Feature-Sliced Design | Масштабируемость, чёткие границы | Требует discipline |

## Decision

Использовать Feature-Sliced Design с элементами Atomic Design для UI-компонентов.

## Consequences

- Чёткое разделение entities/features/widgets/pages.
- Низкая связанность между фичами.
- Требуется ESLint boundaries plugin.

## Related

- `docs/FRONTEND_ARCHITECTURE.md`
- `docs/CODE_STYLE.md`
