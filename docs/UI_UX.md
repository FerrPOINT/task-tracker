# UI/UX Specification — Task Tracker (Jira-like)

## 1. Общие принципы дизайна

- **Тема по умолчанию**: dark.
- **Цветовая палитра**: фон `bg-zinc-950`, карточки `bg-zinc-900`, поднятые поверхности `bg-zinc-800`, границы `border-zinc-700`, акцент `indigo-500`.
- **Типографика**: sans Inter / system-ui, размеры из Tailwind scale.
- **Отступы**: 16px базовый grid, компактный dense layout как в Jira.
- **Контрастность**: доступный WCAG AA для текста.
- **Иконки**: lucide-react, цветные issue type icons.
- **Локализация**: ru / en, LTR.

---

## 2. Глобальный layout

```
+-----------------------------------------------------------+
| ≡ | TaskTracker | Проекты ▼ | Фильтры | Создать ▼ | 🔍 | 🔔 | 👤 |
+-----------------------------------------------------------+
|                                                             |
|  [Sidebar]              [Main Content]                      |
|  - Dashboard            |                                   |
|  - Проекты              |  ...                              |
|  - Мои задачи           |                                   |
|  - Уведомления          |                                   |
|  - Trash                |                                   |
|  - Admin (если есть)    |                                   |
|                         |                                   |
+-----------------------------------------------------------+
```

### Top navigation

- Логотип + название слева.
- `Projects` dropdown: недавние, избранные, all.
- `Create` dropdown: Task, Project, Board, Sprint.
- Search: Cmd+K spotlight, JQL support.
- Notifications bell с badge unread count.
- User avatar → dropdown: profile, settings, logout.

### Sidebar

- Collapsible, 240px по умолчанию.
- Sections: Dashboard, Projects, Filters, Trash.
- Project tree с expand/collapse.

---

## 3. Страница списка проектов

```
+-----------------------------------------------------------+
| Проекты                                    [+ Создать проект]
+-----------------------------------------------------------+
| ▼ Все проекты | ▼ Активные | 🔍 Поиск...                 |
+-----------------------------------------------------------+
|                                                             |
| ┌─────────────┐ ┌─────────────┐ ┌─────────────┐          |
| │ [icon] TT   │ │ [icon] MOB  │ │ [icon] API  │          |
| │ Task Tracker│ │ Mobile App  │ │ Public API  │          |
| │ Scrum       │ │ Kanban      │ │ Basic       │          |
| │ Lead: Ivan  │ │ Lead: Anna  │ │ Lead: Petr  │          |
| │ 42 issues   │ │ 128 issues  │ │ 15 issues   │          |
| └─────────────┘ └─────────────┘ └─────────────┘          |
|                                                             |
+-----------------------------------------------------------+
```

- Card grid / list toggle.
- Карточка: иконка/аватар, название, key, тип, lead, count issues, favorite star.
- Hover: быстрые действия (настройки, архивировать).
- Empty state: CTA создания проекта.

---

## 4. Страница задачи (Issue Detail View)

Главный эталон — Jira issue view: две колонки, левая — контент, правая — мета-поля.

```
+-----------------------------------------------------------+
| TT-42 / Task Tracker > Backlog                           |
|                                                           |
|  [icon Task] Implement user authentication                |
|  Edit  ·  Comment  ·  Assign  ·  More ▼                  |
|                                                           |
| +------------------------------+ +----------------------+ |
| | Описание                     | | Status              ▼ | |
| | [rich text editor / prose]   | | In Progress           | |
| |                              | +----------------------+ |
| |                              | | Assignee            ▼ | |
| |                              | | [👤] Ivan             | |
| |                              | +----------------------+ |
| |                              | | Reporter              | |
| |                              | | [👤] Anna             | |
| |                              | +----------------------+ |
| |                              | | Priority            ▼ | |
| |                              | | High                  | |
| |                              | +----------------------+ |
| |                              | | Labels               + | |
| |                              | | backend, auth         | |
| |                              | +----------------------+ |
| |                              | | Due date              | |
| |                              | | 2026-08-15            | |
| |                              | +----------------------+ |
| |                              | | Components           + | |
| |                              | | API                   | |
| |                              | +----------------------+ |
| |                              | | Fix versions         + | |
| |                              | | v1.0.0                | |
| |                              | +----------------------+ |
| |                              | | Time tracking         | |
| |                              | | 4h spent / 8h est.   | |
| |                              | +----------------------+ |
| |                              | | Watchers              | |
| |                              | | [👤👤] +2             | |
| +------------------------------+ +----------------------+ |
|                                                           |
|  [ Attachments 3 ] [ Links 2 ] [ Sub-tasks 4 ]            |
|                                                           |
|  ┌─────────────────────────────────────────────────────┐ |
|  │ [👤] Ivan · 2 hours ago                              | |
|  │ Implemented login form with validation.             | |
|  │ [image-preview.png]                                | |
|  │                                                    | |
|  │   Reply                                            | |
|  └─────────────────────────────────────────────────────┘ |
|                                                           |
|  [ Activity | Comments | Worklog | History ]             |
|                                                           |
|  · Status changed: To Do → In Progress by Ivan           |
|  · Assignee changed: Unassigned → Ivan by Anna             |
|  · Comment added by Ivan                                   |
|                                                           |
+-----------------------------------------------------------+
```

### 4.1. Header issue

- Breadcrumb: `PROJECT-KEY / Issue Key`.
- Issue type icon + issue key (клик — копировать).
- Summary — inline edit при клике (Enter сохранить, Esc отменить).
- Action bar: Edit, Comment, Assign to me, Status transition dropdown, More (clone, move, delete, watch, vote).

### 4.2. Левая колонка

- **Description**: Tiptap editor, markdown/html просмотр, inline edit.
- **Tabs**: Comments / Activity / Worklog / History.
- **Attachments**: grid preview, drag & drop upload.
- **Issue links**: grouped by link type (blocks, relates to, etc.).
- **Sub-tasks**: список с quick add.

### 4.3. Правая колонка

- **Status**: dropdown с разрешёнными transitions.
- **Assignee**: user picker.
- **Reporter**: read-only.
- **Priority**: colored badge + dropdown.
- **Labels**: inline tag input.
- **Dates**: due date, start date, resolution date.
- **Components**: multi-select.
- **Versions**: fix / affected.
- **Time tracking**: progress bar + log work button.
- **Watchers**: avatars + add.
- **Votes**: count + vote button.

### 4.4. Вкладки

- **Comments**: threaded, rich text, mentions (`@`), attachments per comment.
- **Activity**: системный лог (who did what when).
- **Worklog**: таблица time entries + summary.
- **History**: изменения полей со старыми/новыми значениями.

---

## 5. Kanban board

```
+-----------------------------------------------------------+
| TT Kanban · Backlog 42 · [Sprint 1] active 5 days left   |
| [+ Add column]  ·  ⚙ Filter  ·  👤 Members               |
+-----------------------------------------------------------+
|                                                             |
| ┌───────────┐ ┌───────────┐ ┌───────────┐              |
| │ To Do     │ │ In Progr. │ │ Done      │              |
| │   12      │ │    5      │ │   18      │              |
| │ WIP: 15   │ │ WIP: 5 ⚠️ │ │           │              |
| ├───────────┤ ├───────────┤ ├───────────┤              |
| │ TT-1      │ │ TT-7      │ │ TT-15     │              |
| │ Auth      │ │ OAuth     │ │ Tests     │              |
| │ high      │ │ medium    │ │ low       │              |
| │ [👤]      │ │ [👤👤]    │ │ [👤]      │              |
| │ labels    │ │           │ │           │              |
| │           │ │           │ │           │              |
| │ TT-2      │ │           │ │ TT-16     │              |
| │ Login UI  │ │           │ │ Docs      │              |
| ├───────────┤ ├───────────┤ ├───────────┤              |
| │ + Create  │ │ + Create  │ │ + Create  │              |
| └───────────┘ └───────────┘ └───────────┘              |
|                                                             |
+-----------------------------------------------------------+
```

- Колонки: заголовок с count, WIP limit, цвет.
- Карточка: key, summary, issue type icon, priority badge, assignee avatar, labels, due date.
- Swimlanes: none / assignee / epic.
- Drag & drop между колонками (меняет status) и внутри колонки (меняет rank).
- Quick filters над доской.
- Empty column state: dashed placeholder.
- Column context menu: rename, WIP limit, delete.

---

## 6. Backlog + Sprints

```
+-----------------------------------------------------------+
| Backlog · Task Tracker                                   |
|                                                           |
|  [+ Create sprint]                                       |
|                                                           |
|  ─── Sprint 1 ─── [Start sprint] [Edit] [Delete]         |
|  ☐ TT-10  Auth API            high   [👤]               |
|  ☐ TT-11  Login page          medium [👤]               |
|                                                           |
|  ─── Backlog ─── [+ Create issue]                         |
|  ☐ TT-1   Implement auth      high   [👤]               |
|  ☐ TT-2   Setup CI/CD         medium [👤]               |
|       ...                                                |
+-----------------------------------------------------------+
```

- Иерархия epic → story → sub-task.
- Drag & drop приоритета.
- Кнопки start/complete sprint.
- Velocity indicator (story points committed).

---

## 7. Форма создания задачи

```
+-----------------------------------------------------------+
| Создать задачу                              [×]            |
+-----------------------------------------------------------+
| Project *        [Task Tracker ▼]                        |
| Issue Type *     [Task ▼]                                |
| Summary *        [________________________________]       |
| Description      [Tiptap editor                          ] |
|                  [                                       ] |
| Priority         [Medium ▼]                              |
| Assignee         [Unassigned ▼]                            |
| Labels           [+ add label]                           |
| Due date         [____]                                  |
| Reporter         [me (read-only)]                        |
|                                                           |
| [ Create ]  [ Create and add another ]  [ Cancel ]       |
+-----------------------------------------------------------+
```

- Project + Issue type влияют на видимые поля (screen scheme).
- Валидация inline (garde/Zod).
- Create another — очищает форму, оставляет открытой.

---

## 8. Search / JQL

```
+-----------------------------------------------------------+
| 🔍 Поиск                                                 |
| project = TT AND status != Done                           |
|                                                           |
| [ Search ] [ Save filter ]                                |
|                                                           |
| TT-1  Implement auth        In Progress  Ivan   High      |
| TT-2  Setup CI/CD           To Do        Anna   Medium    |
| ...                                                       |
+-----------------------------------------------------------+
```

- Basic search: project/status/assignee/priority chips.
- Advanced search: JQL textarea с подсветкой/автокомплитом.
- Результаты: таблица или карточки.
- Columns настраиваемые.

---

## 9. Dashboard

```
+-----------------------------------------------------------+
| Team Dashboard                              [+ Add gadget] |
+-----------------------------------------------------------+
| ┌──────────────────┐ ┌─────────────┐ ┌─────────────┐   |
| │ Sprint Burndown  │ │ Open Bugs   │ │ Velocity    │   |
| │ [chart]          │ │ [list]      │ │ [bar chart] │   |
| └──────────────────┘ └─────────────┘ └─────────────┘   |
| ┌──────────────────────────────┐ ┌──────────────────┐   |
| │ Assigned to me               │ │ Recent activity  │   |
| │ ...                          │ │ ...              │   |
| └──────────────────────────────┘ └──────────────────┘   |
+-----------------------------------------------------------+
```

- Grid layout 2–3 columns.
- Gadgets: filter results, burndown, velocity, pie chart by status/assignee.
- Drag & drop rearrange.

---

## 10. Компоненты дизайн-системы

### Buttons

| Вариант | Использование |
|---------|---------------|
| Primary | Create, Save, Start Sprint |
| Secondary | Cancel, More actions |
| Ghost | Text edit, inline add |
| Danger | Delete, archive |
| Icon | Close, collapse, settings |

### Inputs

| Тип | Примечание |
|-----|------------|
| Text | compact height 32px |
| Textarea | auto-resize |
| Select | searchable |
| UserPicker | аватары, фильтр |
| DatePicker | ru/en locale |
| LabelInput | inline tags |
| PriorityBadge | цвет + иконка |
| StatusBadge | цвет фона |

### Feedback

- Toast notifications: success / error / info.
- Loading: skeleton screens, not spinners everywhere.
- Empty states: иллюстрация + CTA.
- Confirmation dialogs для delete/archive.

---

## 11. Интерактивные паттерны

### Inline editing

- Summary, description — клик → режим редактирования → Enter сохранить.
- Поля правой колонки — dropdown/popover → мгновенное сохранение.

### Drag & drop

- Kanban карточки.
- Backlog ordering.
- Dashboard gadgets.
- Attachments reorder.

### Real-time

- WebSocket обновляет:
  - статус карточки на доске
  - счётчики в колонках
  - activity stream
  - comments
- Conflict resolution при одновременном редактировании.

### Keyboard shortcuts

| Комбинация | Действие |
|------------|----------|
| `c` | Create issue |
| `Cmd+K` / `Ctrl+K` | Search |
| `g` then `p` | Go to projects |
| `g` then `i` | Go to issues |
| `e` | Edit issue |
| `/` | Focus search |

---

## 12. Адаптивность

### Desktop (≥1280px)

- Полный двухколоночный issue view.
- Sidebar виден.

### Tablet (768–1279px)

- Sidebar collapsible.
- Issue view: правая колонка становится accordion внизу.

### Mobile (<768px)

- Bottom sheet для фильтров.
- Board: горизонтальный swipe между колонками.
- Issue view: одна колонка, sticky header.

---

## 13. Error / empty / loading states

| Состояние | UI |
|-----------|----|
| Loading | Skeleton screens |
| Empty list | Иконка + текст + CTA |
| No search results | Предложить изменить фильтр |
| 403 | "Нет доступа" с кнопкой назад |
| 404 | "Задача не найдена" |
| Offline | Banner + retry |

---

## 14. Accessibility

- Все интерактивные элементы keyboard-focusable.
- ARIA labels для иконок-кнопок.
- Color не единственный маркер статуса (текст + badge).
- `prefers-reduced-motion` для анимаций.

---

## 15. Страницы и роуты

| Роут | Страница |
|------|----------|
| `/login` | Login |
| `/register` | Register |
| `/` | Dashboard |
| `/projects` | Project list |
| `/projects/new` | Create project |
| `/projects/:id` | Project overview |
| `/projects/:id/settings` | Project settings |
| `/projects/:id/board/:boardId` | Kanban board |
| `/projects/:id/backlog` | Backlog |
| `/issues/:id` | Issue detail |
| `/issues/new?projectId=` | Create issue |
| `/filters` | Saved filters |
| `/filters/:id` | Filter results |
| `/dashboards` | Dashboards |
| `/dashboards/:id` | Dashboard |
| `/admin/users` | User admin |
| `/admin/schemes` | Scheme admin |
| `/settings/profile` | User settings |
| `/notifications` | Notifications |
| `/trash` | Trash |

---

## 16. Материалы для реализации

- Все компоненты строятся на `shadcn/ui` + Tailwind.
- Макеты будут дополнены SVG-мокапами в `docs/assets/ui-mockups/`.
- Цветовые токены и типографика — в `frontend/src/styles/tokens.css`.

---

## 17. SVG-мокапы

См. файлы:
- `docs/assets/ui-mockups/issue-detail.svg`
- `docs/assets/ui-mockups/kanban-board.svg`
- `docs/assets/ui-mockups/project-list.svg`
- `docs/assets/ui-mockups/backlog.svg`
- `docs/assets/ui-mockups/create-issue.svg`
- `docs/assets/ui-mockups/dashboard.svg`

## Appendix: Captured Jira Reference Data

Снято с реального инстанса https://task.wemakedev.ru (read-only, без изменений).

### Issue Types
- `Задача` (standard)
- `Подзадача` (subtask)
- `История` (standard)
- `Ошибка` (standard)
- `Epic` (standard)
- `Маркетинг` (standard)
- `Продукт` (standard)
- `Продажи` (standard)
- `Администрирование` (standard)
- `Анализ` (standard)
- `Техническая` (standard)
- `Таймшит` (standard)

### Statuses
- `Открытый` — К выполнению (new)
- `В работе` — В работе (indeterminate)
- `Сделать` — К выполнению (new)
- `Готово` — Выполнено (done)
- `Backlog` — К выполнению (new)
- `Selected for Development` — К выполнению (new)
- `Тестирование` — В работе (indeterminate)
- `Review` — В работе (indeterminate)
- `Canceled` — Выполнено (done)
- `Готово к тестированию` — К выполнению (new)

### Priorities
- `Highest`
- `High`
- `Medium`
- `Low`
- `Lowest`

### Projects
- `ED` —  Education (software)
- `AAT` — AZHUKOV Azhukov Test (software)
- `PRES` — Presale (business)
- `PT` — Product team (business)
- `ATS` — АТС Релевантер (software)
- `AIPROJ` — ИИ-проекты (software)
- `NEUROKEY` — Нейроключ (software)
- `EDU` — Обучения  (software)
- `REL` — Релевантер (software)
- `NKTIME` — Учет рабочего времени (business)

### Board Column Examples
**Board 10 — Paper1**
- Список задач
- Нужно сделать
- В работе
- Выполнено

**Board 2 — Presale**
- Список задач
- Нужно сделать
- Готовится КП (Без маржи)
- Есть вопросы (к заказчику)
- КП Готово (Без маржи)
- Ожидает ОС (от клиента)
- Согласовано (с заказчиком)
- Пилот
- Выполнено

**Board 4 — Product Team**
- Список задач
- Backlog / Ideas
- To Do
- In Progress
- Review
- Done
- Canceled

### Key Fields in Issue Detail
Системные поля, присутствующие в задачах:
`summary`, `description`, `status`, `assignee`, `reporter`, `creator`, `priority`, `issuetype`, `project`, `labels`, `components`, `fixVersions`, `versions`, `duedate`, `environment`, `timetracking`, `attachment`, `comment`, `worklog`, `issuelinks`, `subtasks`, `votes`, `watches`, `progress`, `aggregateprogress`, `created`, `updated`, `resolution`, `resolutiondate`, `archiveddate`, `archivedby`.

Полный набор данных: `docs/assets/jira-reference/`.
