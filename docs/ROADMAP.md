# Roadmap — Task Tracker

## 1. Overview

План разработки от пустого репозитория до production-ready Jira-like таск-трекера. Каждая фаза — отдельный milestone, заканчивается рабочим коммитом и проверкой.

## 2. Phase 0: Bootstrap (M0)

**Цель**: рабочий каркас, CI, локальный запуск.

- [ ] Rust workspace: `Cargo.toml`, crates `api/app/domain/infra/shared/server/cli`.
- [ ] Frontend: Vite 6.2.0 + React 19.1.0 + TypeScript 5.9.3 + Tailwind CSS 4.1.0 + shadcn/ui.
- [ ] Docker Compose: PostgreSQL 17.6, Redis 8.0, Traefik, backend, frontend.
- [ ] `.env.example`, health endpoints, базовый CI (fmt, clippy, typecheck).
- [ ] `README.md` update с командами запуска.
- [ ] Verification: `docker compose up`, `curl /health`.

## 3. Phase 1: Auth (M1)

**Цель**: регистрация, вход, сессии, пользователи.

- [ ] DB migrations: `users`, `sessions`.
- [ ] Argon2id password hashing.
- [ ] JWT access + httpOnly refresh cookie.
- [ ] Endpoints: `POST /auth/register`, `/auth/login`, `/auth/refresh`, `/auth/logout`.
- [ ] Frontend: login/register pages, auth store, protected routes.
- [ ] Verification: e2e login flow, token refresh, logout.

## 4. Phase 2: Projects (M2)

**Цель**: управление проектами и членами.

- [ ] Migrations: `projects`, `project_members`, `project_role_assignments`, `project_settings`.
- [ ] CRUD projects, project key uniqueness.
- [ ] Default roles: Admin, Member, Viewer.
- [ ] Frontend: project list, create project, project sidebar.
- [ ] Verification: create project, invite member, role checks.

## 5. Phase 3: Issues (M3)

**Цель**: задачи, типы, статусы, workflow.

- [ ] Migrations: `issue_types`, `statuses`, `workflows`, `workflow_statuses`, `workflow_transitions`, `issues`, `issue_status_history`.
- [ ] Default workflow: Open → In Progress → Done.
- [ ] Issue CRUD, key generation (`PROJ-1`).
- [ ] Comments and attachments (file upload).
- [ ] Frontend: issue detail, create issue, comments.
- [ ] Verification: e2e create issue, transition status, comment.

## 6. Phase 4: Kanban Board (M4)

**Цель**: доска с колонками и drag-and-drop.

- [ ] Migrations: `boards`, `board_columns`, `board_quick_filters`.
- [ ] Board config API.
- [ ] WebSocket live updates for board moves.
- [ ] Frontend: kanban board with `@dnd-kit/core`.
- [ ] Verification: screenshots mobile/Full HD/2K, WS real-time.

## 7. Phase 5: Search + Filters (M5)

**Цель**: JQL-поиск и сохранённые фильтры.

- [ ] JQL parser (AST).
- [ ] JQL → SQL builder.
- [ ] Full-text search (`tsvector`).
- [ ] Saved filters CRUD.
- [ ] Frontend: issue navigator, JQL input, filter list.
- [ ] Verification: JQL tests, search performance.

## 8. Phase 6: Notifications + Email (M6)

**Цель**: уведомления и почтовые оповещения.

- [ ] Migrations: `notification_events`, `notification_rules`, `notification_deliveries`.
- [ ] In-app notification center.
- [ ] SMTP integration, email templates.
- [ ] WebSocket push notifications.
- [ ] Verification: trigger notification, receive email.

## 9. Phase 7: Reports (M7)

**Цель**: базовые agile-отчёты.

- [ ] Sprint/Scrum support: `sprints`, `sprint_issues`.
- [ ] Reports: burndown, velocity, cumulative flow, control chart.
- [ ] Frontend: reports hub, charts with `recharts`.
- [ ] Verification: report data accuracy.

## 10. Phase 8: Admin + Settings (M8)

**Цель**: системная админка и настройки проекта.

- [ ] System admin panel: users, groups, global permissions.
- [ ] Project admin: schemes (issue types, workflow, screen, notification).
- [ ] Instance settings: mail, security, backup.
- [ ] Audit log UI.
- [ ] Verification: admin flows, audit log.

## 11. Phase 9: Polish + Production (M9)

**Цель**: production-ready release.

- [ ] Monitoring stack: Prometheus, Grafana, Loki.
- [ ] Backup/restore scripts.
- [ ] Security hardening: CSP, rate limits, audit.
- [ ] Performance optimization: caching, DB indexes, virtualization.
- [ ] Full e2e suite, load tests.
- [ ] Version 0.1.0 release.

## 12. Future (v1.x)

- OAuth/OpenID/LDAP SSO.
- TOTP MFA.
- Email-to-issue.
- CSV import/export.
- Dashboard gadgets.
- Public boards (read-only).
- Mobile app (PWA/capacitor).
- Plugin system.

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
