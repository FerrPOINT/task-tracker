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
- Frontend: публичные компоненты `@pdlc/ui` + Tailwind; предметные композиции остаются в продукте.
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

## Общие поля выбора Base

Сначала искать общий UI в Base: search → show → example. Общие исправления
вносить в Base; продукт передаёт данные, права, преобразования и сохранение.
Для одного известного значения использовать Select, для нескольких — MultiSelect,
для каталога с поиском/пагинацией — SearchableSelect из публичного @pdlc/ui.
Контракт Select/MultiSelect: options, value/defaultValue, onValueChange;
HTML children/option/onChange/multiple/size больше не являются их API.
Видимые raw select и локальные копии оформления не добавлять. Технический form
bridge принадлежит только Base; SSR использует штатный macro и choice-runtime,
явный refresh после программных value/options и native fallback до успешного init.
Правила выбора и подключения: Base docs/platform/CHOICE_CONTROLS.md.
UI/SDK/skills pins независимы; установка и production — отдельные операции.

## Единая дизайн-система Base

Перед созданием UI: pdlc-ui search → show → один example. Button, Input,
Textarea, Checkbox, RadioGroup/Radio, Switch, меню, Card, Table, Disclosure,
Dialog и Toaster/toast использовать из публичного @pdlc/ui. Общий дефект
исправлять в Base; продукт сохраняет данные, права и сохранение. Прямой Sonner,
копии примитивов и продуктовые определения общих tokens не добавлять.
Брендинг применяется общим runtime; draft preview изолируется BrandingScope.
Канонический контракт — Base docs/platform/DESIGN_SYSTEM.md и docs/BRANDING.md.
UI/SDK/skills pins независимы; source-изменения не означают установку.
