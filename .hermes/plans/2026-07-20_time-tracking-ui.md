# План фазы: Time tracking UI

## Цель

Реализовать UI тайм-трекинга для задачи (issue detail): панель в правой колонке с прогрессом, вкладка **Worklog**, диалог **Log work** с ручным вводом/таймером, форматирование времени, i18n `ru`/`en`.

## Текущее состояние (проверено)

- `git status` чистый, последний коммит `03a414f` — документы.
- **Backend**: `backend/Cargo.toml` — placeholder workspace, нет `crates/`, нет миграций, нет API.
- **Frontend**: `frontend/package.json` пустой `{}`, остатки Vue-views (`frontend/src/views/*.vue`), нет React/Vite/Tailwind/shadcn.
- Нет запущенных контейнеров.

## Блокер

Нельзя сделать «только UI» без минимального фронтенд-каркаса. Backend покрывается моками/MSW либо реализуется в той же фазе.

## Варианты

| Вариант | Что делаем | Объём | Результат |
|---|---|---|---|
| **A — UI-first с моками** (рекомендую) | Bootstrap frontend, issue-detail страница, компоненты time tracking, MSW для worklogs | Средний | Рабочий UI на моковых данных, контракт API зафиксирован |
| **B — Вертикальный срез** | Всё из A + backend crates, миграции, реальное API worklogs | Большой | Полностью рабочая фича end-to-end |

По умолчанию предлагаю **вариант A** — он соответствует названию фазы и даёт быстрый проверяемый результат.

---

## План задач (вариант A)

### Task 1: Уточнить контракт тайм-трекинга в документах
**Files:**
- Modify `docs/API.md` — добавить endpoints:
  - `GET /api/v1/issues/{issue_id}/worklogs`
  - `POST /api/v1/issues/{issue_id}/worklogs`
  - `PATCH /api/v1/worklogs/{id}`
  - `DELETE /api/v1/worklogs/{id}`
- Modify `docs/DATA_MODEL.md` — уточнить поля `worklogs` (duration seconds, started_at, comment, remaining_estimate_seconds).
- Modify `docs/USER_STORIES.md` — stories для log/edit/delete worklog.
- Modify `docs/UI_UX.md` — детали панели и вкладки Worklog.

**Verify:** `grep -n worklog docs/API.md docs/DATA_MODEL.md` показывает непустые секции.

---

### Task 2: Bootstrap frontend
**Files:**
- Delete `frontend/src/views/project/ListProjects.vue`
- Delete `frontend/src/views/project/ProjectView.vue`
- Delete `frontend/src/views/tasks/ShowTasks.vue`
- Delete `frontend/src/app/router.tsx` (старое Vue-реликт)
- Rewrite `frontend/package.json`:
  ```json
  {
    "name": "task-tracker-frontend",
    "private": true,
    "version": "0.1.0",
    "type": "module",
    "scripts": {
      "dev": "vite",
      "build": "tsc --noEmit && vite build",
      "typecheck": "tsc --noEmit",
      "test": "vitest run",
      "test:e2e": "playwright test"
    },
    "dependencies": {
      "react": "^19.1.0",
      "react-dom": "^19.1.0",
      "react-router": "^8.1.0",
      "@tanstack/react-query": "^5.74.4",
      "zustand": "^5.0.3",
      "react-hook-form": "^7.55.0",
      "zod": "^4.4.3",
      "@hookform/resolvers": "^4.1.3",
      "lucide-react": "^0.487.0",
      "sonner": "^2.0.3",
      "date-fns": "^4.1.0",
      "class-variance-authority": "^0.7.1",
      "clsx": "^2.1.1",
      "tailwind-merge": "^3.2.0"
    },
    "devDependencies": {
      "typescript": "^5.9.3",
      "vite": "^6.2.0",
      "@vitejs/plugin-react": "^4.4.0",
      "tailwindcss": "^4.1.0",
      "@tailwindcss/vite": "^4.1.0",
      "@types/react": "^19.1.2",
      "@types/react-dom": "^19.1.2",
      "vitest": "^4.1.10",
      "@testing-library/react": "^16.3.0",
      "@playwright/test": "^1.51.1"
    }
  }
  ```
- Create `frontend/index.html`
- Create `frontend/vite.config.ts`
- Create `frontend/tsconfig.json`
- Create `frontend/src/main.tsx`
- Create `frontend/src/app/router.tsx` (React Router, маршрут `/issues/:id`)
- Create `frontend/src/app/App.tsx`
- Update `frontend/src/i18n/locales/ru.json` и `en.json` под React-i18n (`i18next`) или использовать простой контекст.

**Verify:**
```bash
cd frontend
pnpm install
pnpm typecheck
pnpm build
```

---

### Task 3: API/types для worklogs + MSW
**Files:**
- Create `frontend/src/entities/worklog/model.ts`
  ```ts
  export interface Worklog {
    id: string;
    issueId: string;
    userId: string;
    userDisplayName: string;
    timeSpentSeconds: number;
    remainingEstimateSeconds: number | null;
    startedAt: string;
    comment: string | null;
    createdAt: string;
  }
  export interface LogWorkInput {
    timeSpent: string; // "2h 30m"
    remainingEstimate?: string;
    startedAt?: string;
    comment?: string;
  }
  ```
- Create `frontend/src/shared/lib/time.ts` — парсинг/форматирование `2h 30m` ↔ секунды.
- Create `frontend/src/api/worklog.ts` — mock-функции `listWorklogs`, `createWorklog`, `updateWorklog`, `deleteWorklog`.

**Verify:** `pnpm test src/shared/lib/time.test.ts` проходит.

---

### Task 4: Панель Time tracking в правой колонке issue-detail
**Files:**
- Create `frontend/src/features/time-tracking/ui/TimeTrackingPanel.tsx`
- Props: `{ issueId, originalEstimateSeconds, remainingEstimateSeconds, timeSpentSeconds }`
- Показывает: ` spent / estimated / remaining` + progress bar.
- Кнопка **Log work** → открывает диалог.

**Verify:** storybook/ручной рендер на `/issues/1` показывает панель.

---

### Task 5: Вкладка Worklog
**Files:**
- Create `frontend/src/features/time-tracking/ui/WorklogTab.tsx`
- Колонки: пользователь, дата, затраченное время, оставшаяся оценка, комментарий, действия.
- Поддержка edit/delete для своих записей.

**Verify:** скриншот full-page 1920px.

---

### Task 6: Диалог Log work
**Files:**
- Create `frontend/src/features/time-tracking/ui/LogWorkDialog.tsx`
- Form: time spent (`react-hook-form` + zod), remaining estimate, started at, comment.
- Кнопка таймера: start/stop → автозаполнение time spent.

**Verify:** component test открывает диалог, вводит "1h", submit успешен.

---

### Task 7: Страница Issue Detail
**Files:**
- Create `frontend/src/pages/issue-detail/index.tsx`
- Layout: две колонки (левая — описание/tabs, правая — мета-поля).
- Встроить `TimeTrackingPanel` и `WorklogTab`.

**Verify:** Playwright e2e открывает `/issues/1`, видит панель и вкладку.

---

### Task 8: i18n
**Files:**
- Update `frontend/src/i18n/locales/ru.json`, `en.json`.
- Ключи: `timeTracking.title`, `timeTracking.spent`, `timeTracking.estimated`, `timeTracking.remaining`, `timeTracking.logWork`, `worklog.empty`.

**Verify:** переключение языка меняет подписи.

---

### Task 9: Тесты
**Files:**
- `frontend/src/shared/lib/time.test.ts` — парсинг форматов `30m`, `2h`, `1d 4h`.
- `frontend/src/features/time-tracking/ui/LogWorkDialog.test.tsx` — валидация и submit.
- `playwright/tests/issue/time-tracking.spec.ts` — e2e flow.

**Verify:**
```bash
pnpm test
pnpm test:e2e
```

---

### Task 10: Скриншоты и финальная проверка
- Full-page screenshots: 375px, 1920px, 2560px.
- Скриншоты панели, диалога, вкладки Worklog.
- Обновить `docs/UI_UX.md` при необходимости.
- Conventional commits, push.

---

## План задач (вариант B — вертикальный срез)

Если выбираем B, к варианту A добавляется:

1. **Backend bootstrap**: создать `backend/crates/{api,app,domain,infra,shared,server}`.
2. **Migrations**: `users`, `projects`, `issues`, `worklogs` (минимум для фичи).
3. **Domain/Repository**: `Worklog` entity, `WorklogRepository` trait, SeaORM impl.
4. **Service + Controller**: CRUD worklogs, агрегация `time_spent` в issue.
5. **API endpoints**: реализация контракта из Task 1.
6. **Frontend integration**: заменить MSW на реальные запросы к `/api/v1`.
7. **Интеграционные тесты backend** с PostgreSQL testcontainer.
8. **E2E** через Docker Compose: создание issue → лог времени → проверка.

---

## Решение

Предлагаю **вариант A** — UI-first с моками. Он даёт проверяемый UI за одну фазу, а backend worklogs API выделяется в отдельную фазу.

После подтверждения начну с Task 1 (docs) и Task 2 (frontend bootstrap).
