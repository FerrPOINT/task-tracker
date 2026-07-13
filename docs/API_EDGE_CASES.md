# API Edge Cases — Task Tracker

## 1. Overview

Нестандартные сценарии API и ожидаемое поведение.

## 2. Auth Edge Cases

| Scenario | Behavior |
|----------|----------|
| Expired access token | 401, клиент делает refresh |
| Invalid refresh token | 401, требуется повторный login |
| Refresh token reused | Инвалидируем все токены family, security email |
| User deleted during active session | 401 на следующем запросе |
| Password changed elsewhere | Все refresh tokens инвалидируются |

## 3. Concurrent Edits

| Scenario | Behavior |
|----------|----------|
| Two users update same issue | Optimistic locking: 409 с актуальной версией |
| User edits deleted issue | 404 |
| User moves issue to board column without permission | 403 |

## 4. Validation Edge Cases

| Input | Result |
|-------|--------|
| Duplicate project key | 409, error `project_key_exists` |
| Issue type not in project scheme | 422 |
| Status transition not allowed | 422, transition error details |
| Required custom field empty | 422, field-level errors |
| Upload file > 50 MB | 413 |
| JSON body > 10 MB | 413 |

## 5. JQL Edge Cases

| Query | Behavior |
|-------|----------|
| Syntax error | 400 с позицией ошибки |
| Empty result | 200, empty array |
| Field does not exist | 400, unknown field |
| Operator unsupported for field | 422 |
| Result > 1000 | Cursor pagination |

## 6. WebSocket Edge Cases

| Scenario | Behavior |
|----------|----------|
| Client reconnects | Sync missed events via `/sync` |
| Server restarts | Clients reconnect automatically |
| Subscribe to unauthorized channel | 403 close frame |
| Invalid message format | Error event, connection stays |
| Idle connection > 5 min | Server ping, client pong |

## 7. File Upload Edge Cases

| Scenario | Behavior |
|----------|----------|
| Virus found | 400, quarantine file |
| MIME type mismatch | 400 |
| File with same name uploaded | UUID-based storage, dedup по hash |
| Storage quota exceeded | 507 |
| S3 unavailable | Switch to local filesystem (if configured) |

## 8. Background Job Edge Cases

| Scenario | Behavior |
|----------|----------|
| Job fails 3 times | Dead-letter queue |
| Worker crashes during job | Job retried на другом worker |
| Duplicate job submitted | Idempotency key prevents duplicate |
| Long-running job > 5 min | Timeout, partial results logged |

## 9. Rate Limit Edge Cases

| Scenario | Behavior |
|----------|----------|
| Limit exceeded | 429 с `Retry-After` |
| Burst traffic | Token bucket позволяет burst |
| Different limits for user/IP | Применяется более строгий |

## 10. Database Edge Cases

| Scenario | Behavior |
|----------|----------|
| Unique constraint violation | 409 с понятным message |
| Foreign key violation | 422 с указанием зависимости |
| Connection pool exhausted | 503, retry recommended |
| Serialization conflict | 409, retry on client |

## References

- `docs/API.md`
- `docs/ERROR_HANDLING.md`
- `docs/TESTING.md`
- `docs/RESILIENCE.md`
