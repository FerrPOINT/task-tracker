# ADR-0006: JWT Access + httpOnly Refresh Cookie

## Status

Accepted

## Context

Нужна безопасная аутентификация для SPA с возможностью auto-refresh сессии.

## Alternatives Considered

| Option | Pros | Cons |
|--------|------|------|
| Long-lived JWT in localStorage | Просто | XSS-уязвимость |
| Session cookies + Redis | Secure | Сложнее масштабировать |
| JWT access + httpOnly refresh | Баланс security и scalability | Нужен refresh rotation |

## Decision

JWT access token (15 min, memory) + httpOnly refresh cookie (7 days, rotation).

## Consequences

- Access token не хранится в localStorage.
- Refresh cookie защищён от XSS.
- Rotation снижает риск утечки refresh token.
- Нужно корректно обрабатывать 401 и retry queue.

## Related

- `docs/API.md`
- `docs/SYSTEM_ADMIN.md`
