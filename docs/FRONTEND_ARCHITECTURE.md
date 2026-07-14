# Frontend Architecture — Task Tracker

## 1. Overview

Frontend — одностраничное React-приложение на Vite 6.2.0 + TypeScript 5.9.3 + Tailwind CSS 4.1.0 + shadcn/ui. Архитектура построена по методологии **Feature-Sliced Design (FSD)** с элементами **Atomic Design** для презентационных компонентов.

Цели:

- масштабируемость при росте функционала;
- высокая связность внутри фичи, низкая — между фичами;
- переиспользование UI-компонентов;
- чёткое разделение бизнес-логики и UI;
- типобезопасность end-to-end.

## 2. Tech Stack

| Слой | Библиотека |
|------|-----------|
| Framework | React 19.1.0 |
| Build | Vite 6.2.0 |
| Language | TypeScript 5.9.3 |
| Styling | Tailwind CSS 4.1.0 + CSS variables |
| Components | shadcn/ui + Radix primitives |
| State (server) | @tanstack/react-query 5.74.4 |
| State (client) | zustand 5.0.3 |
| Routing | react-router 8.1.0 |
| Forms | react-hook-form 7.55.0 + zod 4.4.3 |
| Tables | @tanstack/react-table |
| DnD | @dnd-kit/core + @dnd-kit/sortable |
| Rich text | @tiptap/react |
| Date | date-fns 4.1.0 |
| Virtualization | @tanstack/react-virtual |
| Charts | recharts |
| Toasts | sonner |
| Tests | vitest 4.1.10 + @testing-library/react 16.3.0 + playwright 1.61.1 |
| E2E | Playwright 1.61.1 |

## 3. Folder Structure

```
frontend/src/
├── app/                    # Инициализация приложения
│   ├── providers.tsx       # Композиция провайдеров
│   ├── router.tsx          # Конфигурация роутов
│   ├── store.ts            # Глобальные zustand-stores
│   ├── query-client.ts     # Конфиг TanStack Query
│   ├── entry-client.tsx    # Точка входа
│   └── entry-server.tsx    # SSR entry (опционально)
├── pages/                  # Страницы приложения
│   ├── dashboard/
│   ├── project-list/
│   ├── project/
│   ├── issue/
│   ├── search/
│   ├── board/
│   ├── backlog/
│   ├── reports/
│   ├── notifications/
│   ├── admin/
│   ├── login/
│   └── register/
├── widgets/                # Самостоятельные UI-блоки
│   ├── header/
│   ├── sidebar/
│   ├── issue-card/
│   ├── board-column/
│   ├── filter-panel/
│   ├── activity-feed/
│   └── dashboard-gadgets/
├── features/               # Пользовательские сценарии
│   ├── create-issue/
│   ├── edit-issue/
│   ├── transition-issue/
│   ├── add-comment/
│   ├── manage-watchers/
│   ├── assign-issue/
│   ├── upload-attachment/
│   ├── filter-issues/
│   ├── configure-board/
│   └── user-preferences/
├── entities/               # Бизнес-сущности
│   ├── issue/
│   ├── project/
│   ├── user/
│   ├── comment/
│   ├── attachment/
│   ├── sprint/
│   ├── board/
│   ├── workflow/
│   └── notification/
├── shared/                 # Переиспользуемый код
│   ├── api/                # axios/fetch instance, interceptors
│   ├── ui/                 # shadcn/ui primitives (Button, Input, Dialog)
│   ├── lib/                # utils (cn, formatters)
│   ├── config/             # env, constants
│   ├── hooks/              # generic hooks (useDebounce, useMediaQuery)
│   ├── types/              # глобальные TypeScript types
│   ├── i18n/               # настройка локализации
│   ├── styles/             # globals.css, tokens.css, animations.css
│   └── tests/              # test utilities, mocks
└── public/
    ├── favicon.svg
    ├── manifest.json
    └── locales/
```

## 4. Feature-Sliced Design (FSD)

### 4.1 Правила зависимостей

```
app → pages → widgets → features → entities → shared
```

- Верхний слой может импортировать нижний.
- Нижний слой **никогда** не импортирует верхний.
- Фичи могут импортировать entities и shared, но не другие features напрямую.
- Cross-feature коммуникация через shared events / store / query keys.

### 4.2 Структура каждого сегмента

```
entities/issue/
├── api/
│   ├── issueApi.ts        # query functions, mutation hooks
│   ├── issueKeys.ts       # TanStack Query keys
│   └── types.ts           # API DTO types
├── model/
│   ├── issueModel.ts      # domain types, helpers
│   ├── issueStatus.ts     # status helpers
│   └── issueSchema.ts     # zod schema
├── ui/
│   ├── IssueSummary.tsx   # small presentational component
│   └── IssueTypeBadge.tsx
├── lib/
│   └── formatIssueKey.ts
└── index.ts               # public API of the segment
```

### 4.3 Public API (index.ts)

Каждый сегмент экспортирует только то, что разрешено:

```ts
// entities/issue/index.ts
export type { Issue, IssueStatus } from "./model/issueModel"
export { IssueSummary } from "./ui/IssueSummary"
export { IssueTypeBadge } from "./ui/IssueTypeBadge"
export { useIssueQuery, useUpdateIssueMutation } from "./api/issueApi"
export { formatIssueKey } from "./lib/formatIssueKey"
```

## 5. Atomic Design for UI Components

### 5.1 Уровни

| Уровень | Папка | Примеры |
|---------|-------|---------|
| Atoms | `shared/ui/` | Button, Input, Badge, Label |
| Molecules | `entities/*/ui/` | IssueSummary, UserAvatar |
| Organisms | `widgets/` | IssueCard, BoardColumn, FilterPanel |
| Templates | `pages/*/ui/` | ProjectLayout, IssueLayout |
| Pages | `pages/` | ProjectPage, IssuePage |

### 5.2 Composition

```tsx
// organisms/IssueCard.tsx
import { Card } from "@/shared/ui/card"
import { IssueSummary } from "@/entities/issue/ui/IssueSummary"
import { IssueTypeBadge } from "@/entities/issue/ui/IssueTypeBadge"
import { AssignIssueFeature } from "@/features/assign-issue"

export function IssueCard({ issue }: { issue: Issue }) {
  return (
    <Card className="p-3">
      <IssueTypeBadge type={issue.type} />
      <IssueSummary summary={issue.summary} />
      <AssignIssueFeature issueId={issue.id} />
    </Card>
  )
}
```

## 6. State Management

### 6.1 Server State — TanStack Query

- Все серверные данные кешируются.
- Query keys структурированы:

```ts
// entities/issue/api/issueKeys.ts
export const issueKeys = {
  all: ["issues"] as const,
  lists: (projectId?: string) => [...issueKeys.all, "list", projectId] as const,
  detail: (id: string) => [...issueKeys.all, "detail", id] as const,
  comments: (id: string) => [...issueKeys.all, "comments", id] as const,
}
```

- Инвалидация после мутаций:

```ts
const mutation = useMutation({
  mutationFn: updateIssue,
  onSuccess: (_, vars) => {
    queryClient.invalidateQueries({ queryKey: issueKeys.detail(vars.id) })
    queryClient.invalidateQueries({ queryKey: issueKeys.lists(vars.projectId) })
  },
})
```

### 6.2 Client State — Zustand

Используется для UI-состояния, не приходящего с сервера:

- `themeStore` — тема;
- `sidebarStore` — свёрнут/развёрнут sidebar;
- `filterStore` — текущий JQL-фильтр;
- `boardStore` — локальные изменения доски (optimistic);
- `notificationStore` — непрочитанные уведомления.

```ts
// app/store.ts
import { create } from "zustand"

interface ThemeState {
  theme: "light" | "dark" | "system"
  setTheme: (theme: ThemeState["theme"]) => void
}

export const useThemeStore = create<ThemeState>((set) => ({
  theme: "dark",
  setTheme: (theme) => set({ theme }),
}))
```

## 7. API Layer

### 7.1 Instance

```ts
// shared/api/apiClient.ts
import axios from "axios"

export const apiClient = axios.create({
  baseURL: import.meta.env.VITE_API_URL,
  timeout: 30000,
})

apiClient.interceptors.request.use((config) => {
  const token = useAuthStore.getState().accessToken
  if (token) config.headers.Authorization = `Bearer ${token}`
  return config
})

apiClient.interceptors.response.use(
  (res) => res,
  async (err) => {
    if (err.response?.status === 401) {
      // refresh flow
    }
    return Promise.reject(err)
  }
)
```

### 7.2 Type Safety

- Генерация API types из OpenAPI через `openapi-typescript`.
- DTO используются в query/mutation hooks.

## 8. Routing

Полная таблица роутов в `docs/ROUTING.md`.

Основные группы:

- Public: `/login`, `/register`, `/forgot-password`.
- App shell: `/` (dashboard).
- Projects: `/projects`, `/projects/:key/*`.
- Issues: `/issues/:key`, `/issues` (search).
- Board: `/boards/:boardId`.
- Backlog: `/projects/:key/backlog`.
- Reports: `/projects/:key/reports/*`.
- Admin: `/admin/*`.
- User: `/notifications`, `/settings`.

## 9. Forms

```tsx
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { createIssueSchema } from "@/entities/issue/model/issueSchema"

export function CreateIssueForm({ projectId }: { projectId: string }) {
  const form = useForm({
    resolver: zodResolver(createIssueSchema),
    defaultValues: { projectId, summary: "", type: "Task" },
  })
  // ...
}
```

## 10. Real-time

- WebSocket connection в `shared/api/wsClient.ts`.
- Подписка на топики по мере открытия страниц.
- Инвалидация TanStack Query при получении `issue_updated`.

## 11. Error Handling

- ErrorBoundary на уровне pages/widgets.
- Query error → toast через sonner.
- Mutation error → inline form error.
- 401 → redirect to login.

Подробнее в `docs/ERROR_HANDLING.md`.

## 12. Testing

### 12.1 Unit / Integration

- Vitest + @testing-library/react.
- Тестируются features и widgets.
- MSW для мока API.

### 12.2 E2E

- Playwright.
- Скриншоты full-page (375 / 1920 / 2560).
- Smoke-тесты для критических путей.

## 13. Performance

- `@tanstack/react-virtual` для длинных списков.
- Lazy loading страниц через `React.lazy`.
- `useMemo` / `useCallback` только при измеренных проблемах.
- Bundle analysis: `vite-bundle-visualizer`.

## 14. Build and Environment

```bash
pnpm install
pnpm dev
pnpm build
pnpm preview
pnpm test
pnpm test:e2e
```

## 15. Code Splitting

```tsx
const ProjectPage = lazy(() => import("./pages/project"))

<Route path="/projects/:key" element={<ProjectPage />} />
```

## 16. Module Boundaries

- ESLint rule `boundaries` или `import/no-restricted-paths` для контроля импортов.
- Запрещено:
  - entities → features/pages/widgets/app;
  - shared → app/pages/widgets/features/entities;
  - cross-import features.
## References

- `docs/ARCHITECTURE.md`
- `docs/UI_LIBRARIES.md`
- `docs/REACT_STYLING.md`
- `docs/DESIGN_TOKENS.md`
