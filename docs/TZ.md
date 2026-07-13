# Полное техническое задание Task Tracker (Jira-like)

## 1. Общее описание

Self-hosted таск-трекер, функционально близкий к Jira Software. Поддерживает проекты, задачи (issues), типы задач, workflow, kanban/scrum-доски, спринты, эпики, кастомные поля, фильтры (JQL), dashboard, уведомления, роли/права, time tracking, релизы, автоматизации и WebSocket-уведомления.

---

## 2. Пользователи и роли

### 2.1. Глобальные роли

| Роль | Описание |
|------|----------|
| **System Admin** | Управление пользователями, настройки инстанса, схемы, роли, лицензии/квоты |
| **Authenticated User** | Любой зарегистрированный пользователь; может создавать персональные фильтры и dashboard |
| **Guest** | Только просмотр по публичным/расшаренным ссылкам (опционально) |

### 2.2. Роли внутри проекта

| Роль | Права по умолчанию |
|------|-------------------|
| **Project Admin** | Настройки проекта, члены, схемы, workflow, доски, релизы |
| **Project Manager** | Создание/редактирование задач, спринты, доски, отчёты |
| **Developer** | Создание задач, выполнение переходов workflow, назначение на себя |
| **Tester** | Просмотр, переходы в статус QA, создание багов |
| **Viewer** | Только просмотр |
| **Custom Role** | Настраиваемый набор permissions |

### 2.3. Project permissions

- View project
- Create issue
- Edit issue
- Delete issue (soft delete / move to trash)
- Transition issue (per workflow)
- Assign issue
- Link issue
- Add comment
- Delete own/all comments
- Add attachment
- Delete attachment
- Manage sprints
- View backlog
- View board
- Admin project
- Manage versions
- Manage components
- Manage filters
- Manage watchers

### 2.4. Global permissions

- Administer system
- Create project
- Bulk change
- Manage users/groups
- Manage permission schemes
- Manage issue type schemes
- Manage workflow schemes
- Manage notification schemes

---

## 3. Проекты

### 3.1. Атрибуты проекта

| Поле | Описание |
|------|----------|
| `id` | UUID v7 |
| `key` | Уникальный короткий ключ: `PROJ`, `TT`, `DEV` (3–10 uppercase) |
| `name` | Название проекта |
| `description` | Описание (rich text) |
| `lead_id` | Владелец/лид проекта |
| `project_type` | `scrum`, `kanban`, `basic` |
| `default_assignee` | `PROJECT_LEAD`, `UNASSIGNED` |
| `avatar_url` | Аватар проекта |
| `status` | `active`, `archived` |
| `created_at`, `updated_at` | Timestamps |

### 3.2. Типы проектов

- **Basic** — простой список задач с фильтрами.
- **Kanban** — доска с колонками, continuous flow, без спринтов.
- **Scrum** — backlog + sprints + boards + burndown/velocity.

### 3.3. Схемы проекта

Каждый проект привязан к наборам схем:

- **Workflow scheme** — workflow для каждого типа задач.
- **Issue type scheme** — какие типы задач доступны.
- **Field configuration scheme** — обязательные/скрытые поля по типам задач.
- **Screen scheme** — какие экраны (наборы полей) при создании/редактировании/просмотре.
- **Permission scheme** — права доступа.
- **Notification scheme** — кто получает уведомления по событиям.

---

## 4. Типы задач (Issue Types)

### 4.1. Системные типы

| Тип | Иконка | Описание |
|-----|--------|----------|
| Task | ☐ | Обычная задача |
| Bug | 🐞 | Дефект |
| Story | 🟢 | User story |
| Epic | ⚡ | Большая фича, объединяет story/task |
| Sub-task | ⤵ | Подзадача |
| Improvement | 🔧 | Улучшение |
| Question | ❓ | Вопрос |

### 4.2. Кастомные типы задач

Project Admin может создавать кастомные issue types с собственным workflow, иконкой, цветом.

### 4.3. Иерархия

```
Epic
├── Story
│   ├── Sub-task
│   └── Sub-task
├── Task
│   └── Sub-task
└── Bug
```

---

## 5. Задачи (Issues)

### 5.1. Системные поля

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | UUID v7 | Уникальный ID |
| `project_id` | UUID | Проект |
| `issue_type_id` | UUID | Тип задачи |
| `key` | string | `PROJ-123` |
| `summary` | string | Заголовок |
| `description` | rich text | Описание |
| `status_id` | UUID | Текущий статус workflow |
| `priority` | enum | Highest, High, Medium, Low, Lowest |
| `assignee_id` | UUID? | Исполнитель |
| `reporter_id` | UUID | Создатель |
| `creator_id` | UUID | Автор |
| `labels` | array text | Метки |
| `components` | array UUID | Компоненты проекта |
| `fix_versions` | array UUID | Релизы/версии |
| `affected_versions` | array UUID | Затронутые версии |
| `epic_id` | UUID? | Epic-родитель |
| `parent_id` | UUID? | Родитель (для sub-task) |
| `rank` | string | Lexorank для сортировки в backlog/board |
| `original_estimate` | interval | Первоначальная оценка времени |
| `remaining_estimate` | interval | Оставшееся время |
| `time_spent` | interval | Затраченное время (агрегат worklog) |
| `due_date` | date | Срок |
| `start_date` | date | Дата начала |
| `resolution` | enum | Fixed, Won't Fix, Duplicate, Cannot Reproduce, Done, etc. |
| `resolution_date` | timestamp | Когда установлена resolution |
| `environment` | text | Окружение |
| `created_at`, `updated_at` | timestamps | — |

### 5.2. Ключ задачи

Автоинкремент внутри проекта: `PROJECT_KEY` + `-` + `sequence`.
Храним `project_issue_counter` таблицу.

### 5.3. Rich text / описание

- HTML/Tiptap JSON
- Поддержка упоминаний `@username`
- Чеклисты
- Вложения-изображения inline

---

## 6. Workflow

### 6.1. Сущности workflow

| Сущность | Описание |
|----------|----------|
| **Workflow** | Именованный набор статусов и переходов |
| **Status** | Состояние задачи: `To Do`, `In Progress`, `Done`, etc. |
| **Transition** | Переход между статусами с условиями/валидаторами/пост-функциями |
| **Transition screen** | Экран, показываемый при переходе |
| **Condition** | Кто может выполнить переход |
| **Validator** | Проверки перед переходом |
| **Post-function** | Действия после перехода: set field, notify, trigger event |

### 6.2. Системные workflow

- **Simple workflow**: To Do → In Progress → Done
- **Scrum workflow**: To Do → In Progress → In Review → Done → Reopened
- **Bug workflow**: Open → Confirmed → In Progress → Fixed → In QA → Closed
- **Approval workflow**: Draft → Pending Approval → Approved → Rejected

### 6.3. Workflow scheme

Привязывает workflow к issue type в рамках проекта.

```
WorkflowScheme {
  project_id,
  mappings: [
    { issue_type_id: "task", workflow_id: "simple" },
    { issue_type_id: "bug", workflow_id: "bug" },
    { issue_type_id: "*", workflow_id: "simple" }
  ]
}
```

### 6.4. Условия переходов

- User has project permission X
- User is assignee / reporter / in role Y
- Field value matches
- Sub-tasks all resolved
- Linked issue status

### 6.5. Пост-функции

- Set field value
- Update issue field (assign to self, set resolution)
- Send notification
- Create event
- Trigger automation
- Add comment
- Reindex for search

---

## 7. Статусы (Statuses)

### 7.1. Системные статусы

| ID | Name | Category | Icon |
|----|------|----------|------|
| todo | To Do | todo | ☐ |
| in_progress | In Progress | in_progress | ▶ |
| in_review | In Review | in_progress | 👁 |
| done | Done | done | ✅ |
| cancelled | Cancelled | done | ❌ |
| reopened | Reopened | todo | ↩ |

### 7.2. Категории статусов

- `todo` — серые
- `in_progress` — синие/жёлтые
- `done` — зелёные

### 7.3. Кастомные статусы

Project Admin может создавать кастомные статусы в workflow.

---

## 8. Kanban / Scrum доски

### 8.1. Board

| Поле | Описание |
|------|----------|
| `id` | UUID |
| `project_id` | Принадлежность проекту |
| `name` | Название |
| `type` | `kanban` / `scrum` |
| `filter_query` | JQL-фильтр задач на доске |
| `column_status_ids` | Порядок колонок |
| `swimlanes` | group by assignee / epic / none |
| `quick_filters` | Список быстрых фильтров |

### 8.2. Колонки

- Колонка = статус workflow (или несколько статусов в одной колонке).
- WIP-limit на колонку.
- Подсветка при превышении WIP.

### 8.3. Карточка задачи на доске

- Key, summary, assignee avatar, priority icon, labels, epic color, story points, due date, attachments count, comments count.

### 8.4. Backlog (Scrum)

- Приоритизированный список задач (rank).
- Drag & drop для сортировки.
- Epic-панели, версии.

### 8.5. Спринты (Scrum)

| Поле | Описание |
|------|----------|
| `id` | UUID |
| `project_id` | Проект |
| `name` | Sprint 1, Sprint 2 |
| `goal` | Цель спринта |
| `start_date` | Дата начала |
| `end_date` | Дата окончания |
| `state` | `future`, `active`, `closed` |
| `issues` | Задачи в спринте |
| `story_points` | Сумма story points |

Действия:
- Create sprint
- Start sprint
- Close sprint (move incomplete to backlog or next sprint)
- Edit sprint
- Delete empty sprint

### 8.6. Burndown / Velocity / Reports

- Burndown chart (story points / issue count / remaining time)
- Velocity chart (completed per sprint)
- Sprint report
- Cumulative flow diagram
- Control chart (cycle/lead time)

---

## 9. Эпики и версии (релизы)

### 9.1. Epic

- Issue type `Epic`.
- Имеет `epic_color`, `epic_name`.
- Связь child issues через `epic_id`.
- Прогресс: % выполненных дочерних задач.

### 9.2. Versions / Fix Versions

| Поле | Описание |
|------|----------|
| `id` | UUID |
| `project_id` | Проект |
| `name` | v1.0.0 |
| `description` | — |
| `start_date`, `release_date` | — |
| `released` | bool |
| `archived` | bool |

- Задачи связываются через `issue_fix_version` / `issue_affected_version`.
- Roadmap view: timeline эпиков/версий.

---

## 10. Кастомные поля

### 10.1. Типы кастомных полей

| Тип | Хранение | UI |
|-----|----------|-----|
| Text (single line) | text | input |
| Text area (multi-line) | text | textarea |
| Number | numeric | number input |
| Date | date | datepicker |
| DateTime | timestamp | datetime picker |
| Select (single) | custom_field_option_id | select |
| Select (multi) | array option ids | multi-select |
| Checkbox | array option ids | checkboxes |
| Radio buttons | option id | radio |
| User picker (single) | user_id | user select |
| User picker (multi) | array user ids | multi user select |
| URL | text | url input |
| Label picker | array text | labels |
| Boolean | bool | toggle |
| Cascading select | parent/child option ids | cascader |

### 10.2. Контекст поля

- Глобальное (Global context) — для всех проектов.
- Проектное (Project context) — для конкретных проектов/типов задач.

### 10.3. Хранение значений

Таблица `issue_custom_field_value`:
- `issue_id`
- `custom_field_id`
- `value_text`
- `value_number`
- `value_date`
- `value_jsonb`

### 10.4. Field configuration

- Обязательные поля по issue type.
- Скрытые поля.
- Default values.

---

## 11. Компоненты проекта

| Поле | Описание |
|------|----------|
| `id` | UUID |
| `project_id` | — |
| `name` | Backend, Frontend, API |
| `description` | — |
| `lead_id` | Ответственный |
| `default_assignee_id` | — |

Используется для группировки задач и авто-назначения.

---

## 12. Связи задач (Issue Links)

### 12.1. Типы связей

| Название | Направленность |
|----------|---------------|
| Blocks / is blocked by | directional |
| Clones / is cloned by | directional |
| Duplicates / is duplicated by | directional |
| Relates to | non-directional |
| Parent / Sub-task | hierarchy |
| Epic link | hierarchy |

### 12.2. Хранение

Таблица `issue_link`:
- `source_issue_id`
- `target_issue_id`
- `link_type_id`
- `created_at`

---

## 13. Комментарии и активность

### 13.1. Комментарии

- Rich text с mentions.
- Inline-вложения.
- Редактирование/удаление своих комментариев.
- Project Admin может удалять любые.

### 13.2. Активность (Activity stream)

- История изменений полей.
- Workflow transitions.
- Worklog.
- Comments.
- Attachments.
- Link changes.

---

## 14. Вложения

| Поле | Описание |
|------|----------|
| `id` | UUID |
| `issue_id` | — |
| `uploader_id` | — |
| `filename` | — |
| `size` | bytes |
| `mime_type` | — |
| `storage_type` | `local`, `s3` |
| `storage_path` | путь |
| `created_at` | — |

- Поддержка drag & drop.
- Превью изображений.
- Inline-вставка в описание/комментарий.
- Хранение в MinIO/S3.

---

## 15. Time Tracking

### 15.1. Поля задачи

- Original estimate
- Remaining estimate
- Time spent (SUM worklogs)

### 15.2. Worklog

| Поле | Описание |
|------|----------|
| `id` | UUID |
| `issue_id` | — |
| `user_id` | Кто списал время |
| `time_spent` | interval |
| `started_at` | Когда выполнялась работа |
| `description` | Что делали |
| `created_at`, `updated_at` | — |

### 15.3. Time tracking reports

- Time spent by user
- Time spent by issue
- Time spent by project
- Sprint capacity vs logged

---

## 16. Поиск и фильтры (JQL)

### 16.1. JQL grammar

```
query      := clause (logical_op clause)*
clause     := field op value
           | field IN (value_list)
           | field IS (EMPTY | NOT EMPTY)
           | field ~ "text"   /* full text */
           | ( query )
logical_op := AND | OR
field      := summary | description | status | assignee | reporter | priority | project | issueType | created | updated | dueDate | labels | components | fixVersion | epic | sprint | text | key | ...
op         := = | != | < | <= | > | >= | ~ | !~ | IN | NOT IN | WAS | WAS IN | WAS NOT IN | CHANGED
value      := string | number | date | datetime | user_ref | project_ref | list
```

### 16.2. Поддерживаемые функции

- `currentUser()`
- `now()`
- `startOfDay()`, `startOfWeek()`, `startOfMonth()`, `startOfYear()`
- `endOfDay()`, `endOfWeek()`, `endOfMonth()`, `endOfYear()`
- `membersOf("role-name")`
- `projectMatch("pattern")`

### 16.3. Примеры

```sql
project = TT AND status IN ("To Do", "In Progress") AND assignee = currentUser()
project = TT AND priority = "High" AND created >= startOfWeek()
text ~ "auth" AND project IN (TT, DEV)
status CHANGED TO "Done" AFTER startOfWeek()
sprint IN ("Sprint 1", "Sprint 2") AND epic = "EPIC-5"
```

### 16.4. Сохранённые фильтры

| Поле | Описание |
|------|----------|
| `id` | UUID |
| `owner_id` | — |
| `name` | — |
| `jql` | строка запроса |
| `description` | — |
| `is_public` | bool |
| `subscriptions` | подписчики |

---

## 17. Dashboard

### 17.1. Dashboard

| Поле | Описание |
|------|----------|
| `id` | UUID |
| `owner_id` | — |
| `name` | — |
| `layout` | JSON конфигурация |
| `is_system` | bool |

### 17.2. Gadgets

| Gadget | Описание |
|--------|----------|
| Filter results | Таблица задач по JQL |
| Pie chart | Распределение по полю |
| Bar chart | Задачи по статусу/assignee |
| Burndown | Для активного спринта |
| Velocity | По завершённым спринтам |
| Assigned to me | Мои задачи |
| Activity stream | Последние события |
| Calendar | Задачи по due date |
| Roadmap | Timeline эпиков/версий |
| WIP limits | Состояние kanban-колонок |

---

## 18. Уведомления

### 18.1. Каналы

- In-app notification
- Email
- WebSocket push

### 18.2. События

- Issue created
- Issue updated
- Status changed
- Assignee changed
- Comment added
- Mentioned in comment/description
- Sprint started/closed
- Issue moved to sprint
- Work logged
- Attachment added
- Issue linked/unlinked

### 18.3. Настройки пользователя

- Watch issue
- Watch project
- Email frequency: instantly / digest / never
- Отключение конкретных типов уведомлений

### 18.4. Notification scheme

Правила: при событии X уведомить:
- current assignee
- reporter
- watchers
- project role
- group
- mentioned users

---

## 19. Автоматизация

### 19.1. Автоматизационные правила

| Компонент | Примеры |
|-----------|---------|
| **Trigger** | Issue created, status changed, comment added, scheduled, webhook |
| **Condition** | issueType = Bug, priority = High, field matches |
| **Action** | Transition issue, assign to user, send email, create sub-task, add comment, set field, create issue in another project, call webhook |

### 19.2. Правила применяются

- Globally
- Per project
- Disabled/enabled

---

## 20. WebSocket / Real-time

### 20.1. События через WebSocket

- issue_updated { issue_id, changed_fields }
- comment_added { issue_id, comment }
- status_changed { issue_id, old_status, new_status }
- sprint_started / sprint_closed
- board_refresh
- notification { user_id, payload }

### 20.2. Топики подписки

- `project:{id}`
- `issue:{id}`
- `board:{id}`
- `user:{id}:notifications`

---

## 21. Import / Export

### 21.1. Import

- CSV (задачи с полями)
- JSON backup
- Jira XML (subset)

### 21.2. Export

- CSV
- JSON
- PDF (отчёты)

---

## 22. Trash / архивация

- Soft delete задач в trash на 30 дней.
- Восстановление Project Admin.
- Автоочистка через cron.
- Архивация проектов.

---

## 23. Audit log

| Поле | Описание |
|------|----------|
| `id` | UUID |
| `actor_id` | Кто |
| `action` | CREATE_ISSUE, UPDATE_ISSUE, DELETE_ISSUE, TRANSITION, LOGIN_FAILED, etc. |
| `entity_type` | issue, project, user, workflow, etc. |
| `entity_id` | — |
| `changes` | JSON diff |
| `ip_address` | — |
| `created_at` | — |

---

## 24. Нефункциональные требования

- P95 API < 200 ms при 100 RPS.
- Full-text search по 100k задач < 500 ms.
- Kanban board 1000 задач < 1 sec initial load.
- WebSocket latency < 100 ms в пределах датацентра.
- i18n: ru, en.
- Dark theme by default.
- Responsive: mobile / tablet / desktop.

---

## 25. User stories по ролям

### System Admin

- Я могу создавать пользователей и назначать глобальные роли.
- Я могу настраивать системные схемы (workflow, issue type, field, permission, notification).
- Я могу просматривать audit log.

### Project Admin

- Я могу создать проект и выбрать его тип (kanban/scrum/basic).
- Я могу приглашать пользователей и назначать роли в проекте.
- Я могу настраивать workflow и issue types для проекта.
- Я могу создавать кастомные поля и экраны.
- Я могу архивировать/удалять проект.

### Project Manager

- Я могу создавать спринты и управлять backlog.
- Я могу назначать задачи и менять приоритеты.
- Я могу просматривать отчёты velocity/burndown.
- Я могу создавать релизы/версии.

### Developer

- Я могу создавать задачи и подзадачи.
- Я могу перемещать задачи по workflow и kanban-доске.
- Я могу логировать время.
- Я могу добавлять комментарии и вложения.

### Tester

- Я могу переводить задачи в QA-статусы.
- Я могу создавать баги.
- Я могу просматривать тестовые задачи.

### Viewer

- Я могу просматривать проекты, доски, задачи (read-only).

---

## 26. Экраны / UI

### 26.1. Глобальные

- Login / Register / Forgot password
- User profile / settings
- System admin panel
- Global search (JQL)
- Dashboard list / Dashboard view
- Filter list / Filter results

### 26.2. Проектные

- Projects list
- Project settings (general, members, roles, schemes, workflow, screens, fields, versions, components, permissions, notifications, automation)
- Backlog (scrum)
- Active sprints / Sprint board
- Kanban board
- Roadmap
- Releases
- Reports

### 26.3. Задачи

- Issue list (table view)
- Issue detail (sidebar + activity)
- Create issue modal/wizard
- Edit issue
- Move issue / clone issue / delete issue
- Link issue dialog
- Log work dialog
- Add comment

---

## 27. API v1 — REST endpoints overview

### 27.1. Auth

- `POST /api/v1/auth/register`
- `POST /api/v1/auth/login`
- `POST /api/v1/auth/refresh`
- `POST /api/v1/auth/logout`
- `POST /api/v1/auth/forgot-password`
- `POST /api/v1/auth/reset-password`

### 27.2. Users

- `GET /api/v1/users/me`
- `PUT /api/v1/users/me`
- `GET /api/v1/users/{id}`
- `GET /api/v1/users`
- `PUT /api/v1/users/{id}/password`
- `POST /api/v1/users/{id}/avatar`

### 27.3. Projects

- `GET /api/v1/projects`
- `POST /api/v1/projects`
- `GET /api/v1/projects/{id}`
- `PUT /api/v1/projects/{id}`
- `DELETE /api/v1/projects/{id}`
- `GET /api/v1/projects/{id}/members`
- `POST /api/v1/projects/{id}/members`
- `PUT /api/v1/projects/{id}/members/{userId}`
- `DELETE /api/v1/projects/{id}/members/{userId}`
- `GET /api/v1/projects/{id}/settings`
- `PUT /api/v1/projects/{id}/settings`

### 27.4. Issue Types

- `GET /api/v1/issue-types`
- `POST /api/v1/issue-types`
- `PUT /api/v1/issue-types/{id}`
- `DELETE /api/v1/issue-types/{id}`

### 27.5. Workflows

- `GET /api/v1/workflows`
- `POST /api/v1/workflows`
- `GET /api/v1/workflows/{id}`
- `PUT /api/v1/workflows/{id}`
- `DELETE /api/v1/workflows/{id}`
- `GET /api/v1/workflows/{id}/transitions`
- `POST /api/v1/workflows/{id}/transitions`
- `POST /api/v1/workflows/{id}/transitions/{transitionId}/execute`

### 27.6. Issues

- `GET /api/v1/issues` (search + JQL)
- `POST /api/v1/issues`
- `GET /api/v1/issues/{id}`
- `PUT /api/v1/issues/{id}`
- `DELETE /api/v1/issues/{id}`
- `POST /api/v1/issues/{id}/assign`
- `POST /api/v1/issues/{id}/transition`
- `POST /api/v1/issues/{id}/watch`
- `POST /api/v1/issues/{id}/vote`
- `GET /api/v1/issues/{id}/activity`
- `POST /api/v1/issues/{id}/clone`
- `POST /api/v1/issues/{id}/move`

### 27.7. Comments

- `GET /api/v1/issues/{id}/comments`
- `POST /api/v1/issues/{id}/comments`
- `PUT /api/v1/issues/{id}/comments/{commentId}`
- `DELETE /api/v1/issues/{id}/comments/{commentId}`

### 27.8. Attachments

- `GET /api/v1/issues/{id}/attachments`
- `POST /api/v1/issues/{id}/attachments`
- `DELETE /api/v1/attachments/{id}`

### 27.9. Worklogs

- `GET /api/v1/issues/{id}/worklogs`
- `POST /api/v1/issues/{id}/worklogs`
- `PUT /api/v1/issues/{id}/worklogs/{worklogId}`
- `DELETE /api/v1/issues/{id}/worklogs/{worklogId}`
- `GET /api/v1/worklogs/reports`

### 27.10. Issue Links

- `GET /api/v1/issues/{id}/links`
- `POST /api/v1/issues/{id}/links`
- `DELETE /api/v1/issue-links/{id}`

### 27.11. Boards

- `GET /api/v1/boards`
- `POST /api/v1/boards`
- `GET /api/v1/boards/{id}`
- `PUT /api/v1/boards/{id}`
- `DELETE /api/v1/boards/{id}`
- `GET /api/v1/boards/{id}/issues`
- `POST /api/v1/boards/{id}/columns/reorder`
- `GET /api/v1/boards/{id}/configuration`
- `PUT /api/v1/boards/{id}/configuration`

### 27.12. Sprints

- `GET /api/v1/sprints`
- `POST /api/v1/sprints`
- `GET /api/v1/sprints/{id}`
- `PUT /api/v1/sprints/{id}`
- `DELETE /api/v1/sprints/{id}`
- `POST /api/v1/sprints/{id}/start`
- `POST /api/v1/sprints/{id}/close`
- `POST /api/v1/sprints/{id}/issues` (add/remove)
- `GET /api/v1/sprints/{id}/burndown`

### 27.13. Versions

- `GET /api/v1/projects/{id}/versions`
- `POST /api/v1/projects/{id}/versions`
- `PUT /api/v1/versions/{id}`
- `DELETE /api/v1/versions/{id}`

### 27.14. Components

- `GET /api/v1/projects/{id}/components`
- `POST /api/v1/projects/{id}/components`
- `PUT /api/v1/components/{id}`
- `DELETE /api/v1/components/{id}`

### 27.15. Custom Fields

- `GET /api/v1/custom-fields`
- `POST /api/v1/custom-fields`
- `PUT /api/v1/custom-fields/{id}`
- `DELETE /api/v1/custom-fields/{id}`
- `GET /api/v1/custom-fields/{id}/options`
- `POST /api/v1/custom-fields/{id}/options`

### 27.16. Filters (JQL)

- `GET /api/v1/filters`
- `POST /api/v1/filters`
- `GET /api/v1/filters/{id}`
- `PUT /api/v1/filters/{id}`
- `DELETE /api/v1/filters/{id}`
- `POST /api/v1/filters/{id}/execute`

### 27.17. Dashboards

- `GET /api/v1/dashboards`
- `POST /api/v1/dashboards`
- `GET /api/v1/dashboards/{id}`
- `PUT /api/v1/dashboards/{id}`
- `DELETE /api/v1/dashboards/{id}`

### 27.18. Notifications

- `GET /api/v1/notifications`
- `PUT /api/v1/notifications/{id}/read`
- `PUT /api/v1/notifications/read-all`
- `GET /api/v1/notifications/settings`
- `PUT /api/v1/notifications/settings`

### 27.19. Reports

- `GET /api/v1/reports/velocity?project={id}`
- `GET /api/v1/reports/burndown?sprint={id}`
- `GET /api/v1/reports/cumulative-flow?project={id}`
- `GET /api/v1/reports/time-tracking?project={id}`

### 27.20. Admin / System

- `GET /api/v1/admin/users`
- `POST /api/v1/admin/users`
- `PUT /api/v1/admin/users/{id}/status`
- `GET /api/v1/admin/audit-log`
- `GET /api/v1/admin/permission-schemes`
- `GET /api/v1/admin/workflow-schemes`
- `GET /api/v1/admin/issue-type-schemes`
- `GET /api/v1/admin/notification-schemes`

### 27.21. WebSocket

- `GET /ws/v1/connect` — handshake, далее subscribe на топики.

### 27.22. Import/Export

- `POST /api/v1/import/csv`
- `POST /api/v1/import/json`
- `POST /api/v1/export/csv`
- `POST /api/v1/export/json`

---

## 28. Локализация и темы

- Языки: `ru`, `en`.
- Темы: `dark` (default), `light`, `system`.
- Date/time форматы по локали.
- First day of week по локали.

---

## 29. Безопасность

- Argon2id для паролей.
- JWT access + httpOnly refresh cookie.
- Rate limiting per IP и per user.
- CSRF защита для cookie-based auth.
- RBAC на всех уровнях.
- Audit log для админ-действий.
- Input validation (garde).
- SQL injection невозможен через SQLx/SeaORM.
- XSS: sanitize HTML на фронте, CSP.

---

## 30. Ограничения и квоты

| Лимит | Значение |
|-------|----------|
| Пользователей в инстансе | конфигурируется |
| Проектов на пользователя | конфигурируется |
| Кастомных полей | 500 |
| Issue types global | 100 |
| Workflow statuses | 200 |
| Transitions per workflow | 100 |
| Аттачмент на issue | 100 |
| Размер аттача | 50 MB |
| Trash retention | 30 дней |

---

## 31. Миграции и совместимость

- Версионирование БД через `refinery`.
- Seed-данные: системные issue types, statuses, workflows, admin user.
- Обратная совместимость API v1 минимум 2 мажорных релиза.

---

## 32. Дорожная карта функционала

### Phase 1 — Foundation

- Auth, users, projects, базовые issue types и workflow.
- CRUD задач, комментарии, attachments.
- Базовый kanban board.
- JQL search.

### Phase 2 — Agile

- Sprints, backlog, scrum board.
- Epic, versions.
- Burndown, velocity.

### Phase 3 — Customization

- Custom fields, screens, field configurations.
- Workflow designer, conditions, validators, post-functions.
- Permission schemes, notification schemes.

### Phase 4 — Scale

- Dashboard, advanced reports.
- Automation rules.
- WebSocket realtime.
- Import/export.
- Audit log.

---

## 33. Подход к реализации

- Начинаем с **Phase 1**.
- Все фичи сначала проектируем в документах, затем API + тесты, затем UI.
- Каждая фича заканчивается e2e тестом и скриншотами (375/1920/2560).
- Код не пишем, пока не зафиксирована дата-модель и API-контракт.
