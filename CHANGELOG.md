# Changelog

Все значимые изменения проекта документируются здесь.
Формат основан на [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/),
версионирование — [SemVer](https://semver.org/lang/ru/).

## [0.2.0] — 2026-08-26

### Added
- Watchers и votes для задач (API + UI + CLI).
- Custom fields: текст/число/дата/select/checkbox, валидация значений по типу поля.
- Components и versions в проектах (API + UI).
- Soft-delete: корзина, restore, purge с каскадным удалением comments/worklogs/attachments/links.
- Notifications: генерация из событий задач, SSE push, настройки email digest.
- CLI: 12 групп команд (auth, projects, issues, sprints, board, comments, labels, search, notifications, reports, admin, members).
- Миграция FK-индексов для 15 внешних ключей.
- 30 backend integration tests (316 total), 28 frontend component tests (88 total).

### Security
- IDOR-фиксы: delete/restore/purge задач, add/remove участника проекта, labels, attachments, issue links, custom fields теперь требуют UserClaims и проверяют права.
- Ошибки Database/Internal больше не раскрывают внутренние детали клиенту (логируются через tracing).
- Board move_issue: валидация принадлежности задачи проекту.

### Fixed
- 21 баг существующего функционала: SSE query keys, search status filter, upload progress, notification mark-as-read, audit log pagination и др.
- CLI: имена полей issue create, URL и метод notification endpoints.
- Issue create возвращает 201 Created.
- Restore неудалённой задачи возвращает 409 Conflict.

### Changed
- Backend декомпозиция: services.rs (2492 строки) → 20 доменных модулей, context.rs (843 строки) → 4 модуля.
- Frontend: единые LoadingState/EmptyState/ErrorState/ConfirmDialog компоненты.
- Native `window.confirm` → shared ConfirmDialog.
- Полный i18n: 393+ ключа RU/EN, захардкоженные строки устранены.
- Accessibility: aria-label на всех icon-кнопках.

### Removed
- Saved filters (API без frontend UI; JQL сохранён — используется search и CLI).
- 20 stale документов, seed demo data, CHANGELOG-заглушка.
- Неиспользуемые зависимости: garde, tokio-test, redis, sqlx (direct), sea-orm из app.

### Docs
- MIGRATIONS.md переписан с Refinery на SeaORM Migrator.
- SECURITY.md: нереализованные controls помечены «не реализовано».
- .env.example и docker-compose.yml: формат env vars `TASKTRACKER_SECTION__KEY`.
- ROUTING.md приведён к 14 реальным маршрутам, DATA_MODEL.md — к фактической схеме.
- API.md: WebSocket секция заменена на SSE.

## [0.1.0] — 2026-08-24

### Added
- Initial release: проекты, kanban board, backlog, sprints, задачи с workflow, labels, comments, attachments, issue links, search (JQL), reports (velocity/burndown/cumulative flow/control chart), auth (JWT + refresh rotation), admin panel, audit log, worklog/time tracking.
- React 19 + Vite 6 + Tailwind 4 frontend, Rust/Axum/SeaORM backend, PostgreSQL.
- Docker Compose: postgres, redis, backend (3456), frontend (19877).
- CI: fmt + clippy + tests + typecheck + lint + build + E2E smoke.
