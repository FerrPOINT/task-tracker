# Frontend UI/UX Library Recommendations

## Goal

Максимально использовать готовые React-библиотеки и компоненты для реализации Jira-like интерфейса. Не писать с нуля то, что уже сделано сообществом.

## Обязательный базовый стек

| Библиотека | Назначение |
|-----------|------------|
| React 19.1.0 | UI runtime |
| TypeScript 5.9.3 | типизация |
| Vite 6.2.0 | сборка |
| Tailwind CSS 4.1.0 | стили |
| shadcn/ui | компоненты (Button, Input, Dialog, Select, Table, Tabs, Accordion, DropdownMenu, Popover, Toast, Tooltip, Badge, Card, Sheet, Skeleton, etc.) |
| lucide-react | иконки |
| @tanstack/react-query 5.74.4 | серверный state |
| zustand 5.0.3 | клиентский state |
| react-router 8.1.0 | роутинг |
| react-hook-form 7.55.0 + zod 4.4.3 | формы + валидация |
| sonner | toast-уведомления |

## Специализированные библиотеки по экранам

### Kanban board — drag & drop

| Библиотека | Плюсы | Минусы | Verdict |
|-----------|-------|--------|---------|
| **@dnd-kit/core + @dnd-kit/sortable** | Современная, доступная, TypeScript-first, лёгкая, хорошо работает с multiple containers, активно поддерживается | Нужно писать обёртки для kanban | ✅ Основной выбор |
| react-beautiful-dnd | Проверенная, красивые анимации | Мертва (Atlassian больше не поддерживает), плохо с horizontal scroll, React 18/19 — проблемы | ❌ Не использовать |
| @hello-pangea/dnd | Форк react-beautiful-dnd с поддержкой React 18 | Всё ещё list-centric, kanban columns — костыли | ⚠️ Запасной |
| react-dnd | Мощная, но сложная | Много boilerplate, старое API | ❌ Не использовать |
| dnd-kit + react-window / @tanstack/react-virtual | Виртуализация длинных колонок | Доп. сложность | ✅ Для production с большими досками |

**Рекомендация:** `@dnd-kit/core`, `@dnd-kit/sortable`, `@dnd-kit/utilities`. Для больших досок добавить `@tanstack/react-virtual`.

### Rich text editor (description, comments)

| Библиотека | Плюсы | Минусы | Verdict |
|-----------|-------|--------|---------|
| **@tiptap/react** | Headless, легко стилизовать под Tailwind/shadcn, расширяемый, collaborative-ready | Нужно собирать toolbar самому | ✅ Основной выбор |
| Slate.js | Гибкий | Низкоуровневый, много кода | ⚠️ Если нужен кастомный редактор |
| Quill | Простой | Устаревший, плохо с React 19.1.0 | ❌ Не использовать |
| Lexical (Meta) | Современный | Менее зрелая экосистема | ⚠️ Альтернатива |
| CKEditor / TinyMCE | Готовый | Платный/тяжёлый | ❌ Не использовать |

**Рекомендация:** `@tiptap/react` + `@tiptap/starter-kit` + `@tiptap/extension-placeholder` + `@tiptap/extension-mention` (для @user). Toolbar из shadcn-кнопок.

### Tables / data grids

| Библиотека | Плюсы | Минусы | Verdict |
|-----------|-------|--------|---------|
| **@tanstack/react-table** | Headless, TypeScript, сортировка, фильтры, пагинация, виртуализация | Нужен UI поверх | ✅ Основной выбор |
| shadcn/ui Table + TanStack Table | Готовая связка | — | ✅ Использовать |
| AG Grid | Мощный | Платный enterprise, тяжёлый | ❌ Для MVP не нужен |
| React Data Grid | Быстрый | Меньше фич | ⚠️ Альтернатива |

### Virtualization

| Библиотека | Verdict |
|-----------|---------|
| **@tanstack/react-virtual** | ✅ Для длинных списков задач, backlog, kanban колонок |
| react-window | ⚠️ Устаревает |

### Charts / reports

| Библиотека | Verdict |
|-----------|---------|
| **recharts** | ✅ Простые burndown/velocity/пироги |
| @tanstack/react-charts / chart.js | ⚠️ Альтернативы |
| visx | ⚠️ Сложнее |

### Date picker / calendar

| Библиотека | Verdict |
|-----------|---------|
| **react-day-picker v9** | ✅ Интегрируется с shadcn Calendar |
| date-fns | ✅ Для форматирования |

### Select / autocomplete / multi-select

| Библиотека | Verdict |
|-----------|---------|
| **cmdk** | ✅ Command palette, issue picker, user picker |
| shadcn/ui Combobox | ✅ Уже на cmdk |
| react-select | ⚠️ Старое, но функциональное |
| downshift | ⚠️ Низкоуровневое |

### Resizable panels / split view

| Библиотека | Verdict |
|-----------|---------|
| **react-resizable-panels** | ✅ Issue navigator split view, detail panel |
| shadcn/ui Resizable | ✅ Обёртка |

### Infinite scroll

| Библиотека | Verdict |
|-----------|---------|
| **@tanstack/react-virtual** + useInfiniteQuery | ✅ Для activity stream, comments |
| react-intersection-observer | ✅ Низкоуровневый хук |

### Markdown / Jira wiki rendering

| Библиотека | Verdict |
|-----------|---------|
| **react-markdown** + rehype/remark | ✅ Для рендера description/comment |
| @tiptap/html | ✅ Для экспорта Tiptap в HTML |

### Avatars / initials

| Библиотека | Verdict |
|-----------|---------|
| shadcn/ui Avatar + radix | ✅ Своя обёртка |
| @radix-ui/react-avatar | ✅ База |

### Drawer / sheets / modals

| Библиотека | Verdict |
|-----------|---------|
| shadcn/ui Sheet, Dialog, Drawer | ✅ Для create issue, filters, mobile |
| vaul | ✅ Drawer для мобильных |

### Tooltips / popovers / dropdowns

| Библиотека | Verdict |
|-----------|---------|
| shadcn/ui Tooltip, Popover, DropdownMenu | ✅ Стандарт |
| @radix-ui/* | ✅ Под капотом shadcn |

### Notifications / toast

| Библиотека | Verdict |
|-----------|---------|
| **sonner** | ✅ Современный toast |

### Forms

| Библиотека | Verdict |
|-----------|---------|
| react-hook-form + zod + shadcn/ui Form | ✅ Стандарт |

### Keyboard shortcuts

| Библиотека | Verdict |
|-----------|---------|
| **react-hotkeys-hook** | ✅ Глобальные hotkeys (c — create, / — search, m — comment, j/k — navigation) |

### Confirmation / dialogs

| Библиотека | Verdict |
|-----------|---------|
| shadcn/ui AlertDialog | ✅ Подтверждение delete/transition |

### Color picker (epic color, label color)

| Библиотека | Verdict |
|-----------|---------|
| **@radix-ui/react-popover** + custom color grid | ✅ Простой color input |
| react-colorful | ⚠️ Лишнее |

### Drag file upload

| Библиотека | Verdict |
|-----------|---------|
| **react-dropzone** | ✅ Attachments |

### Code highlight / JSON view

| Библиотека | Verdict |
|-----------|---------|
| prism-react-renderer / react-syntax-highlighter | ⚠️ По необходимости |

## Итоговый список npm-зависимостей для frontend

```json
{
  "dependencies": {
    "react": "^19.1.0",
    "react-dom": "^19.1.0",
    "react-router": "^8.1.0",
    "@tanstack/react-query": "^5.74.4",
    "zustand": "^5.0.3",
    "react-hook-form": "^7.55.0",
    "zod": "^4.4.3",
    "@hookform/resolvers": "^4.1.3",
    "@tiptap/react": "^2.11.7",
    "@tiptap/starter-kit": "^2.11.7",
    "@tiptap/extension-placeholder": "^2.11.7",
    "@tiptap/extension-mention": "^2.11.7",
    "@dnd-kit/core": "^6.3.1",
    "@dnd-kit/sortable": "^10.0.0",
    "@dnd-kit/utilities": "^3.2.2",
    "@tanstack/react-table": "^8.21.3",
    "@tanstack/react-virtual": "^3.13.6",
    "recharts": "^2.15.2",
    "react-day-picker": "^9.6.7",
    "date-fns": "^4.1.0",
    "cmdk": "^1.1.1",
    "react-resizable-panels": "^2.1.8",
    "react-hotkeys-hook": "^4.6.2",
    "react-dropzone": "^14.3.8",
    "react-markdown": "^10.1.0",
    "remark-gfm": "^4.0.1",
    "sonner": "^2.0.3",
    "lucide-react": "^0.487.0",
    "class-variance-authority": "^0.7.1",
    "clsx": "^2.1.1",
    "tailwind-merge": "^3.2.0"
  },
  "devDependencies": {
    "typescript": "^5.9.3",
    "vite": "^6.2.0",
    "tailwindcss": "^4.1.0",
    "@tailwindcss/vite": "^4.1.0",
    "@types/react": "^19.1.2",
    "@types/react-dom": "^19.1.2",
    "vitest": "^4.1.10",
    "@playwright/test": "^1.51.1"
  }
}
```

## Соответствие экранов → библиотеки

| Экран | Библиотеки |
|-------|-----------|
| Kanban board | `@dnd-kit/core/sortable`, `@tanstack/react-virtual`, shadcn Card/Badge/Avatar/Tooltip |
| Backlog / Sprint | `@dnd-kit/sortable`, `@tanstack/react-table` (для спринтов), shadcn Accordion/Collapsible |
| Issue detail | `@tiptap/react` (description/comments), shadcn Tabs/Card/Avatar/Badge, react-hotkeys-hook |
| Issue navigator | `@tanstack/react-table`, `cmdk`, `react-resizable-panels` |
| Create issue | shadcn Dialog/Sheet/Form/Select/MultiSelect, `@tiptap/react`, `react-dropzone` |
| Dashboard | `recharts`, shadcn Card, `@tanstack/react-table` |
| Reports | `recharts` |
| Search / JQL | `cmdk`, `@tanstack/react-query` |
| User mentions | `@tiptap/extension-mention`, `cmdk` |
| Attachments | `react-dropzone` |
| Date fields | `react-day-picker`, `date-fns` |
| Resizable layouts | `react-resizable-panels` |

## Примечания

- Все компоненты оформляем в стиле **dark-first**, Tailwind + shadcn.
- Цветовые темы реализованы через CSS-переменные: `--color-background`, `--color-surface`, `--color-text-primary`, `--color-accent` и т.д. Поддерживаются три темы: dark, gray, light. Переключатель и сохранение предпочтения в `localStorage`.
- Не используем `@atlaskit/*` — это проприетарные компоненты Atlassian.
- Для accessibility полагаемся на Radix (под капотом shadcn) и `@dnd-kit` (keyboard/sensor support).
- Анимации drag-and-drop — через CSS transforms + `@dnd-kit`.
## References

- `docs/FRONTEND_ARCHITECTURE.md`
- `docs/REACT_STYLING.md`
