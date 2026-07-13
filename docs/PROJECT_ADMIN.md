# Project Administration — Task Tracker

## 1. Project Settings

Каждый проект настраивается через `Project Settings`. Доступно только пользователям с ролью **Project Admin** или **System Admin**.

### 1.1 Основные поля

| Поле | Описание |
|------|----------|
| `name` | Название проекта |
| `key` | Короткий ключ (2-10 заглавных букв), уникальный в инстансе |
| `description` | Описание проекта |
| `project_type_key` | `software` или `business` |
| `lead_id` | Владелец проекта по умолчанию |
| `avatar_url` | URL аватара проекта |
| `category_id` | Категория проекта (опционально) |

### 1.2 Project Types

- `software` — включает доски, спринты, релизы, agile-отчёты.
- `business` — только задачи и базовый workflow.

## 2. Issue Type Scheme

Issue Type Scheme связывает проект с набором типов задач.

### 2.1 Стандартные типы

| Тип | Иконка | Поведение |
|-----|--------|-----------|
| `Task` | галочка | Обычная задача |
| `Story` | звезда | Функциональное требование |
| `Bug` | жук | Ошибка |
| `Epic` | молния | Контейнер для Story/Task |
| `Sub-task` | стрелка вниз | Подзадача |
| `Improvement` | стрелка вверх | Улучшение |

### 2.2 Hierarchy

```
Epic
├── Story
│   ├── Sub-task
├── Task
│   ├── Sub-task
├── Bug
└── Improvement
```

## 3. Workflow Scheme

Workflow Scheme назначает workflow для каждого issue type в проекте.

### 3.1 Default Workflow

| Статус | Категория |
|--------|-----------|
| `To Do` | new |
| `In Progress` | indeterminate |
| `In Review` | indeterminate |
| `Done` | done |
| `Canceled` | done |

### 3.2 Настраиваемые workflow

- Для каждого issue type может быть свой workflow.
- Workflow редактируется в визуальном конструкторе: статусы + переходы.
- Переход может содержать:
  - **conditions** (кто может выполнить);
  - **validators** (обязательные поля);
  - **post-functions** (автоматические действия после перехода);
  - **screen** (экран для ввода полей при переходе).

## 4. Screen Scheme

Screen Scheme определяет, какие поля показываются при:

- создании задачи;
- редактировании задачи;
- просмотре задачи;
- переходе workflow.

### 4.1 Стандартные табы

| Таб | Поля |
|-----|------|
| `Details` | Summary, Type, Status, Priority, Assignee, Reporter, Labels, Components, Versions |
| `Description` | Rich-text description |
| `Comments` | Список комментариев |
| `Attachments` | Вложения |
| `Activity` | История изменений |
| `Links` | Связанные задачи |
| `Work Log` | Учёт времени |

## 5. Field Configuration Scheme

Field Configuration Scheme управляет:

- обязательностью полей;
- видимостью полей;
- renderers (для description/comment);
- масками/валидацией.

### 5.1 Required fields (по умолчанию)

- `summary`
- `issue_type`
- `project`

### 5.2 Optional fields

- `assignee`
- `priority`
- `labels`
- `components`
- `fix_versions`
- `affected_versions`
- `description`
- `attachments`
- `epic_link`
- `sprint`
- `story_points`
- `original_estimate`
- `remaining_estimate`

## 6. Roles and Permissions

### 6.1 Project Roles

| Роль | Описание |
|------|----------|
| `Project Admin` | Полный доступ к настройкам проекта |
| `Project Lead` | Владелец проекта, управление релизами и досками |
| `Developer` | Создание/редактирование задач, переходы |
| `Tester` | Тестирование, переходы в статусы QA |
| `Viewer` | Только чтение |

### 6.2 Permissions

| Permission | Admin | Lead | Developer | Tester | Viewer |
|-----------|-------|------|-----------|--------|--------|
| Administer projects | ✅ | ❌ | ❌ | ❌ | ❌ |
| Browse projects | ✅ | ✅ | ✅ | ✅ | ✅ |
| Create issues | ✅ | ✅ | ✅ | ✅ | ❌ |
| Edit issues | ✅ | ✅ | ✅ | ❌ | ❌ |
| Delete issues | ✅ | ✅ | ❌ | ❌ | ❌ |
| Transition issues | ✅ | ✅ | ✅ | ✅ | ❌ |
| Assign issues | ✅ | ✅ | ✅ | ❌ | ❌ |
| Resolve issues | ✅ | ✅ | ✅ | ✅ | ❌ |
| Close issues | ✅ | ✅ | ✅ | ❌ | ❌ |
| Add comments | ✅ | ✅ | ✅ | ✅ | ❌ |
| Edit all comments | ✅ | ✅ | ❌ | ❌ | ❌ |
| Delete all comments | ✅ | ✅ | ❌ | ❌ | ❌ |
| Create attachments | ✅ | ✅ | ✅ | ✅ | ❌ |
| Delete attachments | ✅ | ✅ | ✅ | ❌ | ❌ |
| View voters/watchers | ✅ | ✅ | ✅ | ✅ | ✅ |
| Manage watchers | ✅ | ✅ | ✅ | ✅ | ❌ |
| View development tools | ✅ | ✅ | ✅ | ✅ | ❌ |
| Schedule sprints | ✅ | ✅ | ❌ | ❌ | ❌ |
| Edit board settings | ✅ | ✅ | ❌ | ❌ | ❌ |

## 7. Components

Компоненты — это логические группы задач внутри проекта.

### 7.1 Поля

- `id`
- `project_id`
- `name`
- `description`
- `lead_id` (опционально)
- `default_assignee_id`

### 7.2 Применение

- Каждая задача может иметь 0..* components.
- Компонент используется в фильтрах, отчётах, досках.

## 8. Versions / Releases

Версии используются для планирования релизов.

### 8.1 Поля

- `id`
- `project_id`
- `name`
- `description`
- `start_date`
- `release_date`
- `released` (bool)
- `archived` (bool)

### 8.2 Связь с задачами

- `fix_versions` — в каких версиях исправлена задача.
- `affected_versions` — в каких версиях обнаружен баг.

### 8.3 Release Hub

- Прогресс-бар по статусам задач в версии.
- Список невыполненных / выполненных / отменённых задач.
- Кнопка "Release version" — переводит все задачи в статус Done.

## 9. Boards

### 9.1 Board Types

- `kanban` — непрерывный поток.
- `scrum` — спринты, бэклог, velocity.

### 9.2 Board Configuration

| Настройка | Описание |
|-----------|----------|
| `columns` | Список колонок и маппинг статусов |
| `wip_limits` | Max задач в колонке |
| `swimlanes` | Группировка (by assignee, by epic, no swimlane) |
| `quick_filters` | Быстрые фильтры доски |
| `estimation_field` | `story_points` или `original_estimate` |
| `card_layout` | Какие поля показывать на карточке |

### 9.3 Columns

Пример для Scrum-доски:

```yaml
columns:
  - name: "To Do"
    statuses: ["Backlog", "Selected for Development"]
    wip_limit: null
  - name: "In Progress"
    statuses: ["In Progress"]
    wip_limit: null
  - name: "Review"
    statuses: ["Review", "Ready for Testing"]
    wip_limit: 5
  - name: "Done"
    statuses: ["Done"]
    wip_limit: null
```

## 10. Project Links

Ссылки на внешние ресурсы проекта:

- Confluence space;
- GitHub/GitLab репозиторий;
- CI/CD dashboard;
- Custom URL.

## 11. Project Import / Export

### 11.1 Export

- JSON dump проекта (задачи, настройки, доски).
- CSV список задач.

### 11.2 Import

- CSV import с маппингом колонок.
- JSON restore (только для System Admin).
