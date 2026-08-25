# Roadmap — Task Tracker

## 1. Overview

План разработки от пустого репозитория до production-ready Jira-like таск-трекера. Каждая фаза — отдельный milestone, заканчивается рабочим коммитом и проверкой.

## Статус (обновлено 2026-08-24)

**Фазы 0–9 завершены** (коммиты `0c942d3` workflow, `f3f8024` attachments, `eb45ae5` labels+links, `d2fed64` coverage gate, `18622c9` SSE real-time, `08f264c`+`e124233` JQL search, `2cd7dfe` JQL/saved-filter tests, `df85a6e` notifications, `001ff30` reports, `646052e` admin).

Реализовано сверх плана фаз 0–4:

- Workflow: глобальные статусы/переходы/типы задач, валидация переходов в `PATCH /issues` и `POST /issues/{id}/transition`.
- Attachments: multipart-загрузка (лимит 25 МБ), дисковое хранилище (`TASKTRACKER_STORAGE`), скачивание, удаление.
- Labels: CRUD на уровне проекта, привязка к задачам.
- Issue links: `blocks` / `duplicates` / `relates`.
- Real-time: SSE `GET /api/v1/events` + инвалидация TanStack Query на фронте (вместо WebSocket из плана — выбран SSE как проще и достаточно для invalidation-модели).
- Тесты: 49 api integration, 40+40 backend unit/domain, 26 vitest, Playwright E2E (smoke/integration/time-tracking/attachments/labels-links/members/realtime); coverage-гейт `scripts/run-e2e-tests.sh` (lines ≥ 77 / regions ≥ 70 / functions ≥ 63).

**Phase 5: Search + Filters (завершена):**

- JQL parser: lexer + recursive descent → AST, 15 unit-тестов (operators, NOT IN, IS EMPTY/NOT EMPTY, chaining, nested parens, error cases).
- JQL → SQL compiler: parameterized SQL, 15 unit-тестов (all fields, IS EMPTY, NOT, labels/sprint EXISTS, timestamp cast, injection safety, UUID validation).
- Full-text search: PostgreSQL `tsvector` + GIN index + triggers (migration 000018).

Отложено до поздних фаз: генерация уведомлений из issue-событий и digest-рассылки, «Корзина»/soft-delete (ссылка из сайдбара убрана), watcher/vote, версии/компоненты, кастомные поля, админка.

## 2. Phase 0: Bootstrap (M0)

**Цель**: рабочий каркас, CI, локальный запуск.

- [x] Rust workspace: `Cargo.toml`, crates `api/app/domain/infra/shared/server/cli`.
- [x] Frontend: Vite 6.2.0 + React 19.1.0 + TypeScript 5.9.3 + Tailwind CSS 4.1.0 + shadcn/ui.
- [x] Docker Compose: PostgreSQL 17.6, Redis 8.0, Traefik, backend, frontend.
- [x] `.env.example`, health endpoints, базовый CI (fmt, clippy, typecheck).
- [x] `README.md` update с командами запуска.
- [x] Verification: `docker compose up`, `curl /health`.

## 3. Phase 1: Auth (M1)

**Цель**: регистрация, вход, сессии, пользователи.

- [x] DB migrations: `users`, `sessions`.
- [x] Argon2id password hashing.
- [x] JWT access + httpOnly refresh cookie.
- [x] Endpoints: `POST /auth/register`, `/auth/login`, `/auth/refresh`, `/auth/logout`.
- [x] Frontend: login/register pages, auth store, protected routes.
- [x] Verification: e2e login flow, token refresh, logout.

## 4. Phase 2: Projects (M2)

**Цель**: управление проектами и членами.

- [x] Migrations: `projects`, `project_members`, `project_role_assignments`, `project_settings`.
- [x] CRUD projects, project key uniqueness.
- [x] Default roles: Admin, Member, Viewer.
- [x] Frontend: project list, create project, project sidebar.
- [x] Verification: create project, invite member, role checks.

## 5. Phase 3: Issues (M3)

**Цель**: задачи, типы, статусы, workflow.

- [x] Migrations: `issue_types`, `statuses`, `workflows`, `workflow_statuses`, `workflow_transitions`, `issues`, `issue_status_history`.
- [x] Default workflow: Open → In Progress → Done.
- [x] Issue CRUD, key generation (`PROJ-1`).
- [x] Comments and attachments (file upload).
- [x] Frontend: issue detail, create issue, comments.
- [x] Verification: e2e create issue, transition status, comment.

## 6. Phase 4: Kanban Board (M4)

**Цель**: доска с колонками и drag-and-drop.

- [x] Migrations: `boards`, `board_columns`, `board_quick_filters`.
- [x] Board config API.
- [x] Live updates for board moves (реализовано через SSE `GET /api/v1/events`, а не WebSocket — см. Статус).
- [x] Frontend: kanban board (HTML5 drag-and-drop без внешних зависимостей).
- [x] Verification: screenshots mobile/Full HD/2K, WS real-time.

## 7. Phase 5: Search (M5)

**Цель**: JQL-поиск.

- [x] JQL parser (AST).
- [x] JQL → SQL builder.
- [x] Full-text search (`tsvector`).
- [x] Frontend: issue navigator, JQL input.
- [x] Verification: JQL tests, search performance.

## 8. Phase 6: Notifications + Email (M6)

**Цель**: уведомления и почтовые оповещения.

- [x] Migrations: `notifications`, `notification_user_settings` (000020).
- [x] In-app notification center: unread list, ownership-safe read/mark-all, user settings, bell dropdown and `/notifications`.
- [x] SMTP integration, HTML/plain-text templates, escaping and disabled-mode no-op.
- [x] OpenAPI + generated frontend client for notification endpoints.
- [x] Verification: repository/service/API/frontend/config/template tests.
- [x] Генерация уведомлений из issue-событий: issue_assigned (create/update), issue_moved (transition), issue_commented (comment). NotificationCreated SSE event → frontend auto-refetch.
- [x] Email digest и production delivery flow: hourly background task собирает непрочитанные уведомления, группирует по recipient, отправляет HTML digest через SMTP, помечает доставленные как прочитанные. respects per-user email_frequency (immediate/hourly/daily), shutdown-cancelled tokio task.
- [x] Real-time push for notification center (NotificationCreated SSE event, frontend auto-refetch notifications).

## 9. Phase 7: Reports (M7)

**Цель**: базовые agile-отчёты.

- [x] Sprint/Scrum support: `sprints`, `sprint_issues` (уже реализовано в Phase 4).
- [x] Reports: velocity, burndown, cumulative flow, control chart.
- [x] Frontend: reports hub, charts with `recharts`.
- [x] Verification: report service unit tests, API integration tests, frontend component tests.

## 10. Phase 8: Admin + Settings (M8)

**Цель**: системная админка и настройки проекта.

- [x] System admin panel: users list, create, activate/deactivate; system-admin role enforcement.
- [x] Instance settings: safe JSON key-value store with admin validation.
- [x] Audit log: append on admin mutations, queryable endpoint + UI.
- [x] Frontend: `/admin` page with Users, Settings, Audit Log tabs.
- [x] Verification: service unit tests, API integration tests, frontend component tests.

## 11. Phase 9: Polish + Production (M9)

**Цель**: production-ready release.

- [x] Security headers: CSP, X-Frame-Options, X-Content-Type-Options, HSTS, Referrer-Policy.
- [x] Rate limiting: auth endpoints 5/15s, general API 60/min per IP (tower-governor).
- [x] Prometheus metrics: `/metrics` endpoint with http_requests_total + duration histogram.
- [x] Performance: DB indexes migration (issues, comments, audit_logs), frontend code-splitting (recharts/vendor/radix/query chunks), React.lazy route-level splitting.
- [x] Backup/restore scripts verified; e2e suite reviewed.
- [x] Version 0.1.0.

## 12. Future (v1.x)

- OAuth/OpenID/LDAP SSO.
- TOTP MFA.
- Email-to-issue.
- CSV import/export.
- Dashboard gadgets.
- Public boards (read-only).
- Mobile app (PWA/capacitor).
- Plugin system.

## 12.1. Implemented post-v0.1.0 (v0.2.0)

- [x] Issue watchers: watch/unwatch, notifications on watched issue changes.
- [x] Issue votes: vote/unvote, vote count.
- [x] Custom fields: project-level definitions (text/number/select/multi-select/date), issue-level values.
- [x] Project components: CRUD, issue component assignment.
- [x] Project versions: CRUD (released/release_date), issue affected/fix version.
- [x] Soft-delete/trash: deleted_at on issues, restore, permanent purge, trash UI.
- [x] CLI: task-tracker binary, 12 command groups, AI-usable skill documentation.

## 13. Definitions of Done

Каждая фаза считается завершённой, когда:

- Код покрыт тестами: unit + integration + critical e2e + coverage gates green.
- Документация обновлена.
- CI green.
- Скриншоты UI (если применимо) приложены.
- Ручная проверка через curl/UI пройдена.

## 14. References

- `docs/TZ.md` — полное ТЗ.
- `docs/ARCHITECTURE.md` — архитектура.
- `docs/DATA_MODEL.md` — дата-модель.
- `docs/DEPLOYMENT.md` — деплой.
- `docs/TESTING.md` — стратегия тестирования.
- `docs/RUNTIME.md` — health probes и graceful shutdown.
- `docs/RESILIENCE.md` — отказоустойчивость.
- `docs/CI_CD.md` — CI/CD pipeline.

## 15. References

- `docs/TZ.md` — техническое задание и scope.
- `docs/ARCHITECTURE.md` — архитектура и стек.
- `docs/USER_STORIES.md` — user stories и use cases.
- `docs/DATA_MODEL.md` — дата-модель.
- `docs/API.md` — REST API спецификация.
- `docs/RELEASE.md` — процесс релизов.
