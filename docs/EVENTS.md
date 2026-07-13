# Event Catalog — Task Tracker

## 1. Overview

Все доменные события, которые генерируют notification, webhook, audit, websocket и background jobs. Единая таблица `events` или per-domain event types.

## 2. Event Structure

```json
{
  "eventId": "uuid",
  "eventType": "issue.created",
  "version": 1,
  "occurredAt": "2026-07-13T12:34:56Z",
  "actorId": "uuid",
  "projectId": "uuid",
  "issueId": "uuid",
  "payload": { ... }
}
```

## 3. Event Types

| Event Type | When | Consumers |
|------------|------|-----------|
| `user.registered` | Новый пользователь | email welcome, audit |
| `user.logged_in` | Успешный вход | audit, suspicious alert |
| `user.password_changed` | Смена пароля | email notification, audit |
| `project.created` | Создан проект | audit, indexing |
| `project.member_added` | Добавлен участник | notification, email, audit |
| `issue.created` | Создана задача | notification, websocket, indexing |
| `issue.updated` | Изменено поле задачи | notification, websocket, changelog |
| `issue.status_changed` | Переход workflow | notification, websocket, board update |
| `issue.assigned` | Назначен assignee | notification, websocket |
| `issue.deleted` | Soft delete | notification, trash, audit |
| `comment.added` | Новый коммент | notification, websocket, mentions |
| `attachment.added` | Новое вложение | notification, thumbnail job |
| `worklog.added` | Ворклог | report update |
| `sprint.started` | Спринт начат | notification, report init |
| `sprint.closed` | Спринт закрыт | report finalize, notification |
| `workflow.transition.executed` | Переход workflow | audit, automation |

## 4. Event Bus

- `tokio::sync::broadcast` in-process.
- Redis Streams для cross-instance.
- `apalis` workers для async consumers.

## 5. Consumer Guarantees

- At-least-once delivery.
- Идемпотентность по `eventId`.
- Retry с exponential backoff.
- Dead-letter queue после 5 неудач.

## 6. Webhook Payload

```json
{
  "event": "issue.created",
  "timestamp": "2026-07-13T12:34:56Z",
  "data": { ...issue dto... },
  "signature": "sha256=..."
}
```

## 7. Audit Mapping

| Event | Audit Action |
|-------|--------------|
| `issue.updated` | `issue:update` |
| `project.member_added` | `project:add_member` |
| `user.password_changed` | `user:change_password` |

## References

- `docs/NOTIFICATIONS.md`
- `docs/WORKFLOW.md`
- `docs/WEBSOCKET_EVENTS.md`
- `docs/SECURITY.md`
