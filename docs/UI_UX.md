# UI/UX Specification — Task Tracker (Jira-like)

## 1. Общие принципы дизайна

- **Тема по умолчанию**: dark.
- **Цветовая палитра**: фон `bg-zinc-950`, карточки `bg-zinc-900`, поднятые поверхности `bg-zinc-800`, границы `border-zinc-700`, акцент `indigo-500`.
- **Типографика**: sans Inter / system-ui, размеры из Tailwind scale.
- **Отступы**: 16px базовый grid, компактный dense layout как в Jira.
- **Контрастность**: доступный WCAG AA для текста.
- **Иконки**: lucide-react, цветные issue type icons.
- **Локализация**: ru / en, LTR.
- **Язык UI по умолчанию**: русский.
- **CSS-токены**: реализованы через `frontend/src/index.css`: `--color-background`, `--color-surface`, `--color-surface-raised`, `--color-border`, `--color-border-strong`, `--color-text-primary`, `--color-text-secondary`, `--color-text-muted`, `--color-accent`, `--color-accent-hover`, `--color-danger`, `--color-success`, `--color-warning`. Имена токенов совпадают в Tailwind (`bg-background`, `text-text-primary`, `bg-accent` и т.д.).
- **Переход между темами**: переключатель в шапке, значение сохраняется в `localStorage` ключ `theme`. Три темы: `dark`, `gray`, `light`.

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
- **Состояния**: loading — skeleton grid 6 карточек; empty — иконка папки + "Нет проектов" + кнопка создания; error — alert с retry.
- **Адаптив**: <768px — 1 колонка, 768–1279px — 2, ≥1280px — 3.

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
- **Time tracking**: progress bar + spent / estimate / remaining + **Log work** button.
  - Показывает: `4h spent / 8h estimated / 4h remaining`.
  - Progress bar: цвет зависит от соотношения spent/estimate (зелёный ≤100%, красный >100%).
  - Клик по кнопке открывает диалог **Log work**.
- **Watchers**: avatars + add.
- **Votes**: count + vote button.

### Time tracking panel (Issue Detail right column)

```
Time tracking
████████████████░░░░░░░░░░░░░░░░░░
4h spent / 8h estimated / 4h remaining
[Log work]
```

- Если `remaining estimate` не задан — показывать только `spent / estimated`.
- Если `time spent > estimated` — progress bar красный, текст `over by Xh`.
- Кнопка **Log work** открывает диалог.

### Log work dialog

```
┌─────────────────────────────┐
│ Log work                     │
├─────────────────────────────┤
│ Time spent      [ 2h 30m ]  │
│ Remaining est.  [ 5h 30m ]  │
│ Started         [2026-07-20] │
│ Comment         [________]  │
│                              │
│ [⏱ Start timer]             │
├─────────────────────────────┤
│ [Cancel] [Save]             │
└─────────────────────────────┘
```

- `Time spent` обязательно, формат `1h 30m` / `45m` / `2d`.
- `Remaining estimate` опционально; если пусто — не менять.
- `Started at` по умолчанию сегодня.
- `Comment` опционально.
- **Start timer**: запускает секундомер, при остановке заполняет `Time spent`.

### 4.4. Вкладки

- **Comments**: threaded, rich text, mentions (`@`), attachments per comment.
- **Activity**: системный лог (who did what when).
- **Worklog**: таблица time entries + summary.
  - Колонки: User, Started, Spent, Remaining estimate, Comment, Actions.
  - Summary внизу: `Total logged: 6h 30m`.
  - Редактирование/удаление только своих записей (или админ/менеджер проекта).
  - **Адаптив**: на мобильных устройствах таблица заменяется на стек карточек; каждая карточка содержит User/Started/Spent/Remaining/Comment и кнопки Edit/Delete.
- **History**: изменения полей со старыми/новыми значениями.

### 4.5. Цветовые темы

Интерфейс поддерживает три темы:

- **Dark** — почти чёрный фон (`#09090b`), поверхности `zinc-900`, акцент periwinkle.
- **Gray** — тёмно-серый фон (`#18181b`), поверхности `zinc-800`, тот же акцент.
- **Light** — светло-серый фон (`#f3f4f6`), белые карточки, тёмный текст.

Переключатель темы находится в шапке. Предпочтение сохраняется в `localStorage` по ключу `theme`. По умолчанию используется тёмная тема. Все UI-токены оформлены через CSS-переменные (`--color-background`, `--color-surface`, `--color-text-primary`, `--color-accent`, и т.д.).

---

## 4a. Worklog Tab Specification

```
+-----------------------------------------------------------+
| Worklog                                                   |
|                                                           |
| User     Started     Spent   Remaining   Comment   Action |
| Ivan     2026-07-20  2h      6h          Login UI   ✎ 🗑  |
| Anna     2026-07-19  4h      2h          API docs   ✎ 🗑  |
|                                                           |
| Total logged: 6h                                          |
+-----------------------------------------------------------+
```

- Сортировка по `startedAt` desc.
- Пагинация не нужна для MVP; scroll при >50 записей.
- Edit inline либо через тот же диалог Log work с prefill.
- Delete с подтверждением AlertDialog.

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
- **Состояния**: loading — skeleton колонки; empty board — CTA добавить первую колонку; WIP limit превышен — бейдж колонки с `warning` цветом.
- **Адаптив**: <768px — горизонтальный swipe между колонками; планшет — горизонтальный scroll.

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
- **Состояния**: loading — skeleton списки; empty sprint — "Перетащите задачи из бэклога"; empty backlog — CTA создать задачу.
- **Адаптив**: <768px — одна колонка, compact rows.

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
- **Состояния**: loading — скелетоны полей; ошибки валидации — под полями красным; success toast.

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
- **Состояния**: loading — skeleton строки; empty results — "Ничего не найдено" + кнопка сброса фильтров; error — alert retry.

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
- **Состояния**: loading — skeleton gadgets; empty dashboard — CTA "Добавить виджет"; error gadget — inline error с retry.

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
|| `g` then `p` | Go to projects |
|| `g` then `i` | Go to issues |
|| `g` then `b` | Go to backlog |
|| `g` then `k` | Go to board |
|| `g` then `d` | Go to dashboard |
|| `e` | Edit issue |
|| `/` | Focus search |
|| `Esc` | Close dialog / cancel edit |
|| `Enter` | Save inline edit / submit form |
|| `?` | Show shortcuts help |

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
- **Touch**: карточки board — long press для меню; swipe right/left для быстрых действий (только для будущих жестов, MVP — кнопки).
- **Keyboard shortcuts на мобильных**: не применяются; spotlight доступен через кнопку поиска.

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
- **Фокус**: видимое кольцо `focus-visible:ring-accent` на всех кнопках и инпутах.
- **Dialogs**: `aria-modal="true"`, `role="dialog"`, trap focus, закрытие по Escape, кнопка Cancel/закрытие всегда доступна.

---

## 15. Страницы и роуты

| Роут | Страница |
|------|----------|
|| `/` | Dashboard |
|| `/login` | Login |
|| `/register` | Register |
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
- **HTML-мокапы**: каждый экран имеет статичную HTML-страницу с переключением тем и навигацией между экранами. См. `docs/assets/ui-mockups/`.
- **React-страницы**: каждый мокап переносится в `frontend/src/pages/<page>/index.tsx` с mock API и роутингом.

---

## 17. SVG-мокапы

См. папки:
- `docs/assets/ui-mockups/` — HTML-мокапы интерфейса Task Tracker. Список файлов:
  - `login.html`
  - `register.html`
  - `dashboard.html`
  - `projects.html`
  - `project-board.html`
  - `project-backlog.html`
  - `search.html`
  - `issue-create.html`
  - `issue-detail.html` (синхронизирован с React-реализацией)
- `docs/assets/jira-samples/` — обезличенные примеры структур Jira (только форматы, без персональных данных)

## Appendix: Jira Structural Samples

Файлы с реальными данными удалены из репозитория. Структурные примеры (issue JSON shape, field schema, board config) будут добавлены в `docs/assets/jira-samples/` в обезличенном виде при необходимости.

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
- `AAT` — [REDACTED] Test (software)
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
## References

- `docs/UI_LIBRARIES.md`
- `docs/FRONTEND_ARCHITECTURE.md`
- `docs/USER_STORIES.md`
