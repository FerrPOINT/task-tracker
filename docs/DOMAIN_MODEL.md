# Domain Model

## 1. Bounded Contexts

| Контекст | Ответственность | Основные агрегаты |
|----------|-----------------|-------------------|
| Identity & Access | Пользователи, сессии, права | User, Session, PermissionScheme |
| Project Management | Проекты, члены команды, роли | Project, ProjectMember, ProjectRole |
| Issue Tracking | Задачи, статусы, transitions | Issue, IssueType, Status, Workflow |
| Workflow Engine | Схемы workflow, conditions, validators | Workflow, Transition, Condition, Validator |
| Configuration | Field configs, screens, custom fields | FieldConfig, Screen, CustomField |
| Collaboration | Комментарии, вложения, mentions | Comment, Attachment |
| Reporting | Dashboards, filters, sprints | Dashboard, SavedFilter, Sprint |
| Notifications | Подписки, события, шаблоны | NotificationScheme, NotificationEvent |

## 2. Главные агрегаты

### User

- Поля: id, username, email, password_hash, display_name, avatar_url, timezone, locale, theme, is_admin, is_active, created_at, updated_at.
- Инварианты:
  - email уникален
  - username уникален, `a-z0-9_`, длина 3–32
  - пароль хранится как Argon2id hash

### Project

- Поля: id, key, name, description, owner_id, lead_id, default_issue_type_id, default_status_id, issue_counter, created_at, updated_at.
- Инварианты:
  - `key` уникален, 2–10 заглавных букв/цифр
  - issue_counter не уменьшается

### Issue

- Поля: id, project_id, issue_key, summary, description, issue_type_id, status_id, assignee_id, reporter_id, priority_id, labels, components, fix_versions, affected_versions, created_at, updated_at.
- Инварианты:
  - `issue_key` уникален внутри проекта (`PROJ-123`)
  - статус должен быть валидным для текущего workflow
  - assignee должен быть членом проекта (если задан)

### Workflow

- Поля: id, name, is_default, project_id (nullable), statuses, transitions, created_at, updated_at.
- Инварианты:
  - ровно один initial status
  - transition graph связный для достижимых статусов
  - conditions/validators не цикличны

### Comment

- Поля: id, issue_id, author_id, body, mentions, created_at, updated_at.
- Инварианты:
  - body не пустой после trim

## 3. Value Objects

| VO | Пример | Ограничения |
|---|---|---|
| IssueKey | `PROJ-42` | `[PROJECT-KEY]-[NUMBER]` |
| Email | `user@example.com` | RFC-5322 subset |
| Password | — | min 8 chars, 1 upper, 1 lower, 1 digit |
| Color | `#1a2b3c` | hex 6 |
| JQLQuery | `status = open AND assignee = me` | grammar в `docs/JQL.md` |

## 4. Domain Events

| Событие | Контекст | Потребители |
|---|---|---|
| IssueCreated | Issue Tracking | Notifications, Search index, Activity log |
| IssueStatusChanged | Issue Tracking | WebSocket, Reports, History |
| CommentAdded | Collaboration | Notifications, WebSocket |
| ProjectMemberAdded | Project Management | Permissions cache, Notifications |
| UserRegistered | Identity | Welcome email, Audit log |

## 5. Anti-corruption Layers

- Импорт/миграция из внешних систем — отдельный `Migration` контекст, маппинг через адаптеры.
- Email/Slack интеграции — абстракция `NotificationChannel`, реализации в инфраструктуре.

## 6. Invariants и бизнес-правила

- Переход статуса issue возможен только если выполнены:
  1. transition существует в текущем workflow
  2. пользователь имеет право `TRANSITION_ISSUE`
  3. все validators возвращают `true`
  4. post-functions выполнены успешно
- Нельзя удалить issue type, если на него ссылаются активные issue.
- Нельзя удалить project, не удалив/архивировав все issue.

## 7. References

- `docs/ARCHITECTURE.md` — общая архитектура.
- `docs/DATA_MODEL.md` — физическая модель данных.
- `docs/WORKFLOW.md` — workflow engine.
- `docs/JQL.md` — поисковый DSL.
- `docs/API.md` — REST API.
