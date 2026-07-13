# Notifications — Task Tracker

## 1. Events

Каждое событие имеет `event_type`, `actor_id`, `project_id`, `issue_id` (опционально).

### 1.1 Issue Events

| Event | Описание |
|-------|----------|
| `issue_created` | Создана задача |
| `issue_updated` | Изменено поле задачи |
| `issue_deleted` | Задача удалена |
| `issue_assigned` | Назначен assignee |
| `issue_status_changed` | Изменён статус |
| `issue_resolved` | Задача решена |
| `issue_closed` | Задача закрыта |
| `issue_reopened` | Задача переоткрыта |
| `issue_commented` | Добавлен комментарий |
| `issue_comment_edited` | Комментарий отредактирован |
| `issue_comment_deleted` | Комментарий удалён |
| `issue_attachment_added` | Добавлено вложение |
| `issue_link_created` | Создана связь |
| `issue_worklog_logged` | Записано время |

### 1.2 Project Events

| Event | Описание |
|-------|----------|
| `project_created` | Создан проект |
| `project_updated` | Обновлён проект |
| `project_deleted` | Удалён проект |
| `version_released` | Версия выпущена |
| `sprint_started` | Спринт начат |
| `sprint_completed` | Спринт завершён |

### 1.3 User Events

| Event | Описание |
|-------|----------|
| `user_mentioned` | Пользователь упомянут (@username) |
| `user_assigned` | Пользователю назначена задача |
| `user_watching` | Изменения в отслеживаемой задаче |

## 2. Notification Channels

| Channel | Описание |
|---------|----------|
| `in_app` | Внутреннее уведомление, bell icon, badge |
| `email` | Email по SMTP |
| `webhook` | HTTP webhook |
| `slack` | Slack integration (опционально) |
| `telegram` | Telegram bot (опционально) |

## 3. Notification Rules

### 3.1 Default Rules

- Assignee получает уведомления об изменениях задачи.
- Reporter получает уведомления о комментариях и статусах.
- Watchers получают все изменения.
- Упомянутый пользователь (`@username`) получает `user_mentioned`.
- Project Admin получает уведомления о событиях проекта.

### 3.2 User Preferences

Каждый пользователь настраивает для себя:

| Событие | in_app | email |
|---------|--------|-------|
| Issue assigned to me | ✅ | ✅ |
| Issue I watch changed | ✅ | ✅ |
| Issue I reported changed | ✅ | ✅ |
| Someone mentions me | ✅ | ✅ |
| Sprint events | ✅ | ❌ |
| Project events | ✅ | ❌ |
| Daily digest | ❌ | ❌ |

## 4. Notification Scheme

Notification Scheme привязывается к проекту и определяет, кто получает какие уведомления.

### 4.1 Recipients

- Current user
- Reporter
- Assignee
- Watchers
- Project role
- Group
- Single user
- Mentioned user

### 4.2 Templates

| Channel | Format |
|---------|--------|
| in_app | Title + body + link + actor avatar |
| email | HTML template + plain text fallback |

### 4.3 Template Variables

- `{{actor.display_name}}`
- `{{issue.key}}`
- `{{issue.summary}}`
- `{{event.name}}`
- `{{project.name}}`
- `{{comment.body}}`
- `{{changelog.field}}`
- `{{changelog.from}}`
- `{{changelog.to}}`

## 5. In-App Notifications UI

### 5.1 Bell Icon

- Значок в topbar.
- Badge с количеством непрочитанных.
- Dropdown с последними 10 уведомлениями.

### 5.2 Notification Center

- Страница `/notifications`.
- Табы: All / Unread / Mentions.
- Фильтры: by project, by date.
- Mark all as read.
- Preferences button.

### 5.3 Real-time Delivery

- WebSocket channel `/ws/notifications/{user_id}`.
- Новое уведомление приходит мгновенно.

## 6. Email Notifications

### 6.1 Batching

- Мгновенные email для критичных событий (assign, mention).
- Digest email раз в 15 минут / час / день для остальных.

### 6.2 Rate Limiting

- Max 1 email в 5 минут на одно событие для одного пользователя.
- Дублирующие события группируются.

## 7. Webhook Notifications

### 7.1 Webhook Payload

```json
{
  "event": "issue_status_changed",
  "timestamp": "2026-07-13T12:00:00Z",
  "actor": {"id": "uuid", "display_name": "[REDACTED]"},
  "project": {"id": "uuid", "key": "PROJ"},
  "issue": {"id": "uuid", "key": "PROJ-123"},
  "data": {
    "field": "status",
    "from": "To Do",
    "to": "In Progress"
  }
}
```

### 7.2 Retry Policy

- 3 retries с экспоненциальным backoff.
- Webhook отключается после 5xx подряд.

## 8. Mention System

### 8.1 Syntax

- `@username` в description и comment.
- Autocomplete при вводе `@`.

### 8.2 Rendering

- Mention отображается как badge с аватаром.
- Ссылка на профиль пользователя.

## 9. Digest

### 9.1 Daily Digest

- Список изменений по watched/assigned задачам за день.
- Отправляется в 9:00 по таймзоне пользователя.

### 9.2 Weekly Digest

- Сводка по спринтам, завершённым задачам, новым багам.

## 10. Data Model

### 10.1 Tables

- `notification_events` — типы событий.
- `notification_schemes` — схемы проектов.
- `notification_scheme_items` — правила в схеме.
- `notifications` — отправленные уведомления.
- `notification_preferences` — настройки пользователя.
- `webhooks` — глобальные webhooks.
- `webhook_deliveries` — история доставок.
