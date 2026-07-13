# Workflow Engine — Specification

## 1. Концепция

Workflow определяет жизненный цикл задачи через статусы и переходы. Каждый issue type в проекте может иметь свой workflow через workflow scheme.

---

## 2. Сущности

### 2.1. Status

| Атрибут | Описание |
|---------|----------|
| id | UUID |
| name | To Do, In Progress, Done |
| category | todo / in_progress / done |
| color | hex |
| icon | emoji/URL |

### 2.2. Workflow

| Атрибут | Описание |
|---------|----------|
| id | UUID |
| name | Simple Workflow |
| statuses | список статусов |
| transitions | список переходов |
| is_system | bool |

### 2.3. Transition

| Атрибут | Описание |
|---------|----------|
| id | UUID |
| name | Start Progress |
| from_status_id | UUID или null (global transition) |
| to_status_id | UUID |
| screen_id | UUID (опциональный экран для ввода полей) |
| conditions | список условий |
| validators | список валидаторов |
| post_functions | список пост-функций |

---

## 3. Conditions (кто может выполнить)

| Тип | Config | Пример |
|-----|--------|--------|
| `permission` | `{ "permission_key": "TRANSITION_ISSUE" }` | У пользователя есть право |
| `role` | `{ "role_name": "developer" }` | Пользователь в роли |
| `assignee` | `{}` | Только текущий assignee |
| `reporter` | `{}` | Только reporter |
| `field_value` | `{ "field": "priority", "op": "=", "value": "high" }` | Значение поля |
| `expression` | `{ "expr": "issue.assignee == currentUser" }` | Произвольное выражение |

Логика внутри группы conditions: **AND**. Можно добавить группу OR через `condition_group`.

---

## 4. Validators (проверки перед переходом)

| Тип | Config | Пример |
|-----|--------|--------|
| `field_required` | `{ "field": "resolution" }` | Поле обязательно при переходе |
| `field_pattern` | `{ "field": "comment", "regex": ".+" }` | Комментарий не пустой |
| `linked_issue_status` | `{ "link_type": "blocks", "status_category": "done" }` | Все blocked задачи done |
| `subtasks_resolved` | `{}` | Все sub-tasks в done |
| `expression` | `{ "expr": "issue.time_spent > 0" }` | Произвольное условие |

---

## 5. Post-functions (действия после перехода)

| Тип | Config | Пример |
|-----|--------|--------|
| `set_field` | `{ "field": "assignee", "value": "current_user" }` | Назначить на себя |
| `clear_field` | `{ "field": "remaining_estimate" }` | Очистить поле |
| `add_comment` | `{ "body": "Transitioned to {to_status_name}" }` | Авто-комментарий |
| `send_notification` | `{ "event_type": "status_changed" }` | Уведомление |
| `trigger_event` | `{ "event_type": "status_changed" }` | Системное событие |
| `create_subtask` | `{ "issue_type_id": "uuid", "summary_template": "Review {parent.key}" }` | Создать sub-task |
| `call_webhook` | `{ "url": "...", "payload": {} }` | Вызвать внешний webhook |
| `set_resolution` | `{ "resolution": "Fixed" }` | Установить resolution |

---

## 6. Workflow scheme

Mapping issue type → workflow:

```json
{
  "project_id": "uuid",
  "workflow_scheme_id": "uuid",
  "mappings": [
    { "issue_type_id": "uuid-task", "workflow_id": "uuid-simple" },
    { "issue_type_id": "uuid-bug", "workflow_id": "uuid-bug" },
    { "issue_type_id": null, "workflow_id": "uuid-simple" }
  ]
}
```

`issue_type_id: null` — default workflow.

---

## 7. Процесс выполнения transition

1. Загрузить issue.
2. Определить workflow по issue type + workflow scheme.
3. Найти transition по `from_status_id` → `to_status_id` (или по transition_id).
4. Проверить conditions.
5. Проверить validators.
6. Выполнить post-functions.
7. Обновить issue.status_id.
8. Записать `issue_status_history`.
9. Создать activity log.
10. Отправить WebSocket-событие.
11. Уведомить подписчиков.

---

## 8. Системные workflow

### 8.1. Simple Workflow

```
To Do → In Progress → Done
```

### 8.2. Bug Workflow

```
Open → Confirmed → In Progress → Fixed → In QA → Closed
```

### 8.3. Scrum Workflow

```
Backlog → To Do → In Progress → In Review → Done → Reopened
```

### 8.4. Approval Workflow

```
Draft → Pending Approval → Approved / Rejected
```

---

## 9. Категории статусов

| Category | Цвет по умолчанию | Описание |
|----------|-------------------|----------|
| todo | серый | Ещё не начато |
| in_progress | синий | В работе |
| done | зелёный | Завершено |

Категория используется для:
- board column grouping
- burndown calculation
- cumulative flow diagram
- resolution detection (done → auto resolution)

---

## 10. UI Workflow Designer

- Граф nodes (statuses) + edges (transitions).
- Drag & drop для переупорядочивания.
- Панель свойств transition: conditions/validators/post-functions.
- Валидация циклов.
- Предпросмотр workflow.

---

## 11. API Workflow

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | /workflows | Список |
| POST | /workflows | Создать |
| GET | /workflows/{id} | Получить |
| PUT | /workflows/{id} | Обновить |
| DELETE | /workflows/{id} | Удалить (если не system) |
| GET | /workflows/{id}/transitions | Переходы |
| POST | /workflows/{id}/transitions | Добавить переход |
| PUT | /workflows/{id}/transitions/{tid} | Обновить переход |
| DELETE | /workflows/{id}/transitions/{tid} | Удалить переход |
| GET | /statuses | Список статусов |
| POST | /statuses | Создать статус |

---

## 12. Примеры post-functions JSON

```json
[
  {
    "type": "set_field",
    "config": { "field": "assignee", "value": "{current_user_id}" }
  },
  {
    "type": "add_comment",
    "config": { "body": "Moved to {to_status.name}" }
  },
  {
    "type": "trigger_event",
    "config": { "event_type": "status_changed" }
  }
]
```

---

## 13. Безопасность и ограничения

- System workflow нельзя удалить.
- Workflow нельзя удалить, если он используется в active workflow scheme.
- При изменении active workflow создаётся draft, который нужно опубликовать.
- Переход должен вести к статусу, входящему в workflow.
- Запрещены переходы, приводящие к бесконечному циклу (проверка DAG при сохранении).
## References

- `docs/DATA_MODEL.md`
- `docs/PROJECT_ADMIN.md`
- `docs/API.md`
