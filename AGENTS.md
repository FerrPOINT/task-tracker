# AGENTS.md — Task Tracker

## Репозиторий

- **GitHub**: `git@github.com:FerrPOINT/task-tracker.git`
- **Стек**: backend Rust (Axum + SeaORM + PostgreSQL), frontend React 19.1.0 + Vite 6.2.0 + Tailwind CSS 4.1.0
- **Env prefix**: `TASKTRACKER_`
- **Публичные порты по умолчанию**: frontend docker `19877`, backend `3456`. PostgreSQL и Redis внутренние (compose-сеть), наружу не публикуются

## Правила работы

### 1. Перед началом работы

1. Прочитать `docs/TZ.md`, `docs/ARCHITECTURE.md`, `docs/DATA_MODEL.md`.
2. Проверить текущее состояние ветки: `git status`.
3. Составить план, показать пользователю, получить подтверждение.

### 2. Код

- Backend: слоистая архитектура `controller → service → repository`.
- DI через `AppContext`.
- Все публичные API покрыты OpenAPI через `utoipa-axum`.
- Rust-хендлеры и DTO — единственный источник правды для схемы; фронт генерирует клиент из `openapi/openapi.json`.
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

- [x] Все тесты проходят.
- [x] Линтеры (`clippy`, `eslint`, `prettier`) чистые.
- [x] Документация актуальна.
- [x] Коммиты запушены в `origin/main`.
- [x] Пользователь увидел результат (скриншот / curl / лог).

## Контакты

- Техлид: Александр Жуков.
- Основной язык общения и документов: русский.

## References

- `docs/ARCHITECTURE.md`
- `docs/CODE_STYLE.md`
- `docs/TESTING.md`

## Base и воспроизводимость

См. [BASE_INTEGRATION](docs/BASE_INTEGRATION.md). Обязательны pinned Base SHA,
locked/frozen зависимости и проверка актуальных checkout до сборки.
Rust build toolchain 1.88.0, Node 22.20.0, pnpm 10.28.1; MSRV отдельно.

## Local Docker ownership

Every local container uses real Compose. Permanent projects are sdlc1, sdlc2
and sdlc-common. Temporary sdlc-qa-/sdlc-build- projects require task/purpose,
explicit endpoint and scoped cleanup in finally. Direct docker run/create,
fake Compose labels and global prune are forbidden. Runtime data, snapshots,
keys and shared caches are preserved. Base owns tools and LOCAL_GROUPS rules.

## Термины PDLC и общие компоненты

Перед изменением платформенных имён прочитайте [интеграцию с Base](docs/PDLC_BASE_INTEGRATION.md) и канонические TERMS/BRANDING.
Для UI: `pnpm --dir frontend exec pdlc-ui search "задача"` → `show ExportName` → один нужный `example`. В Workflow SSR используйте штатный exporter Base.
Не переносите продуктовые запросы, DTO и права в общий UI. SDK/UI/skills pins независимы; новое имя платформы не меняет SDLC IDs/scopes или Hermes upstream.

## Единое пространство PDLC

[Согласованный план](docs/plans/unified-spaces-20261019.md) вводит M:N и общий read admission. До полного
совместимого cohort это цель, не установленное поведение. Строгие V1/V2 и
applied migrations сохранять; ProjectRef/SpaceRef/V3 отдельны. Полный gate на
интегрированной вехе, в ходе реализации scoped checks. Runtime не активировать
по готовности одного продукта; не выбирать первый Project/Space неявно.


Контекст центрального запроса содержит immutable identity, свежую глобальную
роль, product scopes и исходный срок допуска. Не повышать роль в JIT и не
использовать исторический `is_system_admin` как доказательство роли платформы.
Task-local proof не передаётся в spawned jobs автоматически; потоки используют
явно захваченный исходный proof, а фоновые операции — свой ограниченный протокол.
Исторические уведомления и аудит не сохраняют доступ к данным отозванного
проекта. Ограничивать выборку и счётчики до pagination; неизвестный ресурс не
считать глобальным. События пользователей и system settings классифицировать
по их существующим глобальным правилам, отдельно от проектного содержимого.

Правила текущей транзакционной записи:
[PROJECT_CONTENT_AUTHORIZATION_V2](docs/PROJECT_CONTENT_AUTHORIZATION_V2.md).
Не использовать raw concrete connection как обход ordinary content guards.
Owner/reconcile и machine artifact/job ports имеют отдельные typed proofs;
read-only proof не становится правом записи. После owner-v2 adoption legacy
writer не считается совместимым даже до включения общего policy flag.
