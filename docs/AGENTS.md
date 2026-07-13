# AGENTS.md — Task Tracker

## Репозиторий

- **GitHub**: `git@github.com:FerrPOINT/task-tracker.git`
- **Стек**: backend Rust (Axum + SeaORM + PostgreSQL), frontend React 19 + Vite 6 + Tailwind 4
- **Порт**: `19876`
- **Env prefix**: `TASKTRACKER_`

## Правила работы

### 1. Перед началом работы

1. Прочитать `docs/TZ.md`, `docs/ARCHITECTURE.md`, `docs/DATA_MODEL.md`.
2. Проверить текущее состояние ветки: `git status`.
3. Составить план, показать пользователю, получить подтверждение.

### 2. Код

- Backend: слоистая архитектура `controller → service → repository`.
- DI через `AppContext` / `shaku`.
- Все публичные API покрыты OpenAPI через `utoipa-axum`.
- Все endpoint тестируются интеграционно через testcontainers.
- Frontend: компоненты на `shadcn/ui` + Tailwind.
- Состояние: серверное — `@tanstack/react-query`, клиентское — `zustand`.
- Формы — `react-hook-form` + `zod`.

### 3. Коммиты

- Conventional commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`).
- Один коммит = одна логическая единица.
- Не amend/squash без явного запроса.
- Push только после проверки `cargo test`, `pnpm typecheck`, `pnpm test`.

### 4. Тестирование

- Backend: `cargo test`, интеграционные тесты с PostgreSQL testcontainer.
- Frontend: Vitest + Playwright.
- После UI-изменений — скриншоты full-page (375 / 1920 / 2560).
- Все новые endpoint — curl-проверка.

### 5. Документация

- При изменении API обновлять `docs/API.md`.
- При изменении дата-модели обновлять `docs/DATA_MODEL.md`.
- При новом функционале добавлять/обновлять `docs/TZ.md`.
- Любые неочевидные решения фиксировать в `docs/ARCHITECTURE.md`.

### 6. Безопасность

- Никогда не коммитить credentials, токены, пароли, email коллег, реальные данные клиентов.
- Все secrets — через env vars.
- Перед push проверять, что в diff нет чувствительных данных.

### 7. Docker

- Сборка: `docker compose build`.
- Пересоздание контейнера: `docker compose up -d` (не `docker compose restart`).
- Проверка: `docker compose ps` и health endpoint.

### 8. Проверка перед завершением

- [ ] Все тесты проходят.
- [ ] Линтеры (`clippy`, `eslint`, `prettier`) чистые.
- [ ] Документация актуальна.
- [ ] Коммиты запушены в `origin/main`.
- [ ] Пользователь увидел результат (скриншот / curl / лог).

## Контакты

- Техлид: Александр Жуков.
- Основной язык общения и документов: русский.
