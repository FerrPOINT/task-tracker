# Обзор необходимых библиотек для Task Tracker

Собраны обязательные библиотеки для backend (Rust) и frontend (React), с пояснением зачем нужны и какую роль выполняют в таск-трекере.

---

## Backend: Rust

### 1. Web-фреймворк / HTTP

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`axum`](https://crates.io/crates/axum) | 0.8.9 | HTTP-маршруты, extractors, маршрутизация | Официальный фреймворк Tokio/Hyper; лучшая интеграция с Tower middleware; экосистема набирает обороты в 2026; удобен для слоёв Controller → Mapper → Service → Repository |
| [`tokio`](https://crates.io/crates/tokio) | 1.52.3 | Асинхронный runtime | Стандарт индустрии; async/await, пулы задач, таймеры |
| [`tower`](https://crates.io/crates/tower) | 0.5.2 | Middleware абстракция (Service/ServiceBuilder) | Общий слой middleware для rate limit, auth, tracing, CORS |
| [`tower-http`](https://crates.io/crates/tower-http) | 0.7.0 | Готовые HTTP middleware | CORS, compression, trace, auth, validate-request |
| [`hyper`](https://crates.io/crates/hyper) | 1.6.0 | HTTP-драйвер под капотом axum | Стандарт HTTP-имплементации в Rust |

### 2. База данных / ORM / миграции

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`sea-orm`](https://crates.io/crates/sea-orm) | 2.0.x | Async ORM, миграции, relations | Близка к Spring JPA: Entity → Repository; relations, migrations, eager/lazy loading; SeaORM 2.0 выпущен в январе 2026, production-ready |
| [`sqlx`](https://crates.io/crates/sqlx) | 0.9.0 | Raw SQL + compile-time проверка | Для сложных JQL-запросов, отчётов, миграций; fallback под SeaORM |
| [`refinery`](https://crates.io/crates/refinery) | 0.8.14 | Чистые SQL-миграции | Простые файлы `V1__init.sql` → применение на старте; альтернатива SeaORM Migrator |
| [`deadpool-postgres`](https://crates.io/crates/deadpool-postgres) | 0.14.0 | Пул соединений PostgreSQL | Можно использовать совместно с sqlx; в SeaORM пул идёт из коробки |

### 3. Аутентификация / авторизация / безопасность

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`argon2`](https://crates.io/crates/argon2) | 0.6.0-pre.1 | Хеширование паролей (argon2id) | Рекомендуется OWASP; победитель предыдущих сравнений хешей |
| [`jsonwebtoken`](https://crates.io/crates/jsonwebtoken) | 10.4.0 | JWT access/refresh токены | Стандарт для stateless auth; версия 10.x стабильна |
| [`ring`](https://crates.io/crates/ring) | 0.17.14 | Криптографические примитивы | Используется jsonwebtoken под капотом; можно отдельно для случайных данных |

### 4. Валидация

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`garde`](https://crates.io/crates/garde) | 0.23.0 | Валидация DTO | Аннотации на структурах, как Spring `@Valid`; быстрее и проще validator |
| [`validator`](https://crates.io/crates/validator) | 0.20.0 | Альтернатива garde | Если понадобится более широкий набор валидаторов |

### 5. Конфигурация / переменные окружения

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`figment`](https://crates.io/crates/figment) | 0.10.19 | Unified config: TOML + ENV + defaults | Аналог Spring `@ConfigurationProperties`; префикс `TASKTRACKER_` |
| [`dotenvy`](https://crates.io/crates/dotenvy) | 0.15.7 | `.env` файл в dev-режиме | Удобно для локальной разработки |

### 6. Логирование / observability

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`tracing`](https://crates.io/crates/tracing) | 0.1.44 | Структурированные логи + spans | Аналог Spring Boot SLF4J/Logback; интеграция с OpenTelemetry |
| [`tracing-subscriber`](https://crates.io/crates/tracing-subscriber) | 0.3.19 | Форматтеры (JSON, pretty) | JSON для продакшена, pretty для dev |
| [`tracing-bunyan-formatter`](https://crates.io/crates/tracing-bunyan-formatter) | 0.3.10 | Bunyan JSON формат | Если нужен единый формат с фронтендом/Node |

### 7. OpenAPI / документация API

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`utoipa`](https://crates.io/crates/utoipa) | 5.5.0 | Генерация OpenAPI из Rust-типов | Удобно для Axum; Swagger UI; аналог SpringDoc |
| [`utoipa-axum`](https://crates.io/crates/utoipa-axum) | 0.2.0 | Интеграция utoipa с axum | Router + OpenAPI из одних хендлеров |
| [`utoipa-swagger-ui`](https://crates.io/crates/utoipa-swagger-ui) | 9.0.1 | Swagger UI endpoint | `/swagger-ui` для разработки |

### 8. Фоновые задачи / очереди / CRON

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`apalis`](https://crates.io/crates/apalis) | 0.7.4 | Async job queue (PostgreSQL/Redis/SQLite) | Аналог Spring `@Scheduled` + `@Async`; retry, backoff, cron, concurrency |
| [`apalis-cron`](https://crates.io/crates/apalis-cron) | 0.7.4 | Cron-триггеры | Для регулярных задач: напоминания, отчёты, очистка |

### 9. Кэширование

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`moka`](https://crates.io/crates/moka) | 0.12.15 | In-memory cache | Аналог Spring Cache (Caffeine); TTL, size eviction, async-aware |
| [`redis`](https://crates.io/crates/redis) | 1.3.0 | Redis клиент | Distributed cache + pub/sub для инвалидации + WebSocket state |
| [`deadpool-redis`](https://crates.io/crates/deadpool-redis) | 0.20.0 | Пул Redis-соединений | Для высоких нагрузок на WebSocket/session store |

### 10. Email / уведомления

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`lettre`](https://crates.io/crates/lettre) | 0.11.22 | SMTP/HTTP email sending | Аналог Spring Mail; async, connection pool, поддержка SMTP и Sendgrid/Postmark через SMTP relay |
| [`maud`](https://crates.io/crates/maud) | 0.26.0 | Compile-time HTML templates | Для email-писем; безопасно, быстро |

### 11. WebSocket / realtime

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`tokio-tungstenite`](https://crates.io/crates/tokio-tungstenite) | 0.26.2 | WebSocket server | Аналог Spring WebSocket; де-факто стандарт в экосистеме Tokio |

### 12. HTTP-клиент / исходящие запросы

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`reqwest`](https://crates.io/crates/reqwest) | 0.13.4 | HTTP-клиент | Аналог Spring `RestTemplate`/`WebClient`; async, middleware, JSON |

### 13. Rate limiting / API gateway

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`tower_governor`](https://crates.io/crates/tower_governor) | 0.8.0 | Rate limiter middleware | Governor + Tower; per-IP, per-key, burst |

### 14. Метрики / health

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`metrics`](https://crates.io/crates/metrics) | 0.24.6 | Производственные метрики | Аналог Micrometer; counters, histograms |
| [`metrics-exporter-prometheus`](https://crates.io/crates/metrics-exporter-prometheus) | 0.17.0 | Prometheus endpoint | `/actuator/prometheus` по сути |

### 15. Тестирование

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`mockall`](https://crates.io/crates/mockall) | 0.15.0 | Mock-объекты для trait | Аналог Mockito; автоматические моки для сервисов и репозиториев |
| [`testcontainers`](https://crates.io/crates/testcontainers) | 0.27.3 | Интеграционные тесты с Docker | PostgreSQL/Redis в тестах; поднимается перед cargo test |
| [`wiremock`](https://crates.io/crates/wiremock) | 0.6.3 | Mock HTTP-сервер | Для тестирования исходящих клиентов |
| [`tokio-test`](https://crates.io/crates/tokio-test) | 0.4.4 | Тестирование async-кода | `#[tokio::test]`, time manipulation |
| [`rstest`](https://crates.io/crates/rstest) | 0.25.0 | Parametrized tests | Spring `@ParameterizedTest` на Rust |
| [`assert-json-diff`](https://crates.io/crates/assert-json-diff) | 2.0.2 | Сравнение JSON в тестах | Для API-ответов |

### 16. Утилиты

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`serde`](https://crates.io/crates/serde) | 1.0.219 | Сериализация/десериализация JSON и др. | Стандарт |
| [`serde_json`](https://crates.io/crates/serde_json) | 1.0.140 | JSON | Стандарт |
| [`chrono`](https://crates.io/crates/chrono) | 0.4.41 | Даты/время | Аналог Java `java.time` |
| [`uuid`](https://crates.io/crates/uuid) | 1.17.0 | UUID v4/v7 | ID задач, проектов; v7 — сортируемый по времени |
| [`thiserror`](https://crates.io/crates/thiserror) | 2.0.12 | Собственные ошибки | Кодогенерация Error impl |
| [`anyhow`](https://crates.io/crates/anyhow) | 1.0.98 | Удобный Result<T> | Для CLI, bin-кода, тестов |
| [`cargo-workspace`](https://doc.rust-lang.org/cargo/reference/workspaces.html) | — | Workspace для backend/cli | Разделение на crate'ы: `api`, `domain`, `infra`, `cli`, `shared` |

---

## Frontend: React 19 + TypeScript

### 1. Базовый стек

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`react`](https://www.npmjs.com/package/react) | 19.1.0 | UI-библиотека | Последний stable; Server Components, Actions |
| [`react-dom`](https://www.npmjs.com/package/react-dom) | 19.1.0 | DOM-рендеринг | Пара react |
| [`vite`](https://www.npmjs.com/package/vite) | 6.2.0 | Сборщик / dev-server | Быстрее Webpack; HMR; ESM-first |
| [`typescript`](https://www.npmjs.com/package/typescript) | 5.9.3 | Типизация | Последний стабильный |
| [`@vitejs/plugin-react-swc`](https://www.npmjs.com/package/@vitejs/plugin-react-swc) | 3.9.0 | SWC-based React plugin | Быстрая компиляция |

### 2. Стилизация

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`tailwindcss`](https://www.npmjs.com/package/tailwindcss) | 4.1.0 | Утилитарный CSS | v4 с новым движком; темизация, dark mode |
| [`@tailwindcss/vite`](https://www.npmjs.com/package/@tailwindcss/vite) | 4.1.0 | Интеграция Tailwind 4 + Vite | Официальный плагин |
| [`clsx`](https://www.npmjs.com/package/clsx) | 2.1.1 | Условные className | Стандарт |
| [`tailwind-merge`](https://www.npmjs.com/package/tailwind-merge) | 3.2.0 | Merge классов без конфликтов | Вместе с `cn()` хелпером |
| [`class-variance-authority`](https://www.npmjs.com/package/class-variance-authority) | 0.7.1 | Варианты компонентов | Как shadcn-варианты |

### 3. Компоненты / дизайн-система

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`shadcn/ui`](https://ui.shadcn.com/) | latest | Headless компоненты + CLI | Копирует компоненты в проект; полный контроль над стилем; лучший выбор для кастомного дизайна |
| [`@radix-ui/react-*`](https://www.radix-ui.com/) | 1.2.0 | Headless primitives | Dialog, Dropdown, Select, Tooltip, Tabs, Accordion — доступность из коробки |
| [`lucide-react`](https://www.npmjs.com/package/lucide-react) | 0.487.0 | Иконки | Лёгкие, стильные |

### 4. Маршрутизация

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`react-router`](https://www.npmjs.com/package/react-router) | 8.1.0 | Маршрутизация + data API | Loaders/actions/data-mode — удобно для таск-трекера с prefetch |

### 5. Управление состоянием

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`@tanstack/react-query`](https://www.npmjs.com/package/@tanstack/react-query) | 5.74.4 | Серверное состояние, кэширование, мутации | Аналог React Query; автоматическая синхронизация данных задач/проектов |
| [`zustand`](https://www.npmjs.com/package/zustand) | 5.0.3 | Глобальное клиентское состояние | Аналог Redux, но минималистичный; UI state: sidebar, модалки, тема |
| [`@tanstack/react-store`](https://www.npmjs.com/package/@tanstack/react-store) | 0.7.0 | Лёгкий сигнал-подобный стейт | Альтернатива zustand, если захочется уйти от Flux |

### 6. Формы / валидация

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`react-hook-form`](https://www.npmjs.com/package/react-hook-form) | 7.55.0 | Управление формами | Минимум ререндеров; интеграция с Zod |
| [`zod`](https://www.npmjs.com/package/zod) | 4.4.3 | Схемы валидации | Одна схема для фронта и (потенциально) backend |
| [`@hookform/resolvers`](https://www.npmjs.com/package/@hookform/resolvers) | 5.0.1 | Мост RHF + Zod | Стандарт |

### 7. Drag & Drop / Kanban

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`@dnd-kit/core`](https://www.npmjs.com/package/@dnd-kit/core) | 6.3.1 | Модульный DnD | Современная замена react-beautiful-dnd; поддержка мышь/тач/клавиатура, virtual lists |
| [`@dnd-kit/sortable`](https://www.npmjs.com/package/@dnd-kit/sortable) | 10.0.0 | Sortable lists | Колонки и карточки kanban |
| [`@dnd-kit/utilities`](https://www.npmjs.com/package/@dnd-kit/utilities) | 3.2.2 | Хелперы dnd-kit | Стандарт |

### 8. Редактор описаний (rich text)

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`@tiptap/react`](https://www.npmjs.com/package/@tiptap/react) | 2.11.7 | Headless rich text editor | Markdown/HTML, mentions, задачи-чеклисты; хорошо контролируемый |
| [`@tiptap/extension-mention`](https://www.npmjs.com/package/@tiptap/extension-mention) | 2.11.7 | Упоминания пользователей | `@user` в комментариях/описании |
| [`@tiptap/extension-task-list`](https://www.npmjs.com/package/@tiptap/extension-task-list) | 2.11.7 | Чеклисты внутри задачи | Подзадачи |

### 9. Даты / календари

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`date-fns`](https://www.npmjs.com/package/date-fns) | 4.1.0 | Манипуляции с датами | Tree-shakeable, immutable |
| [`react-day-picker`](https://www.npmjs.com/package/react-day-picker) | 9.6.0 | Календарь / datepicker | Лёгкий, доступный, кастомизируемый |

### 10. Локализация

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`i18next`](https://www.npmjs.com/package/i18next) | 25.0.0 | Фреймворк i18n | Стандарт; ru/en |
| [`react-i18next`](https://www.npmjs.com/package/react-i18next) | 15.5.0 | React-обёртка | Хуки `useTranslation` |

### 11. Уведомления / UX

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`sonner`](https://www.npmjs.com/package/sonner) | 2.0.3 | Toast-уведомления | Лёгкие, стильные; ошибки/успехи |
| [`@tanstack/react-virtual`](https://www.npmjs.com/package/@tanstack/react-virtual) | 3.13.6 | Виртуализация списков | Для длинных списков задач и kanban |

### 12. Тестирование

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`vitest`](https://www.npmjs.com/package/vitest) | 4.1.10 | Unit / компонентные тесты | Быстрый, Vite-native |
| [`@testing-library/react`](https://www.npmjs.com/package/@testing-library/react) | 16.3.0 | Тестирование React-компонентов | User-centric подход |
| [`@testing-library/jest-dom`](https://www.npmjs.com/package/@testing-library/jest-dom) | 6.6.3 | Matchers для DOM | `toBeVisible`, `toHaveTextContent` |
| [`@testing-library/user-event`](https://www.npmjs.com/package/@testing-library/user-event) | 14.6.1 | Симуляция пользователя | Типинг, клики, DnD |
| [`@playwright/test`](https://www.npmjs.com/package/@playwright/test) | 1.61.1 | E2E / скриншоты | Critical path: авторизация, проекты, задачи, kanban |
| [`msw`](https://www.npmjs.com/package/msw) | 2.7.5 | Mock Service Worker | Мок API в unit-тестах и storybook |

### 13. Утилиты

| Библиотека | Версия | Назначение | Почему она |
|------------|--------|------------|------------|
| [`axios`](https://www.npmjs.com/package/axios) | 1.8.4 | HTTP-клиент (альтернатива fetch) | Интерцепторы, отмена запросов; или `fetch` + TanStack Query |
| [`@tanstack/react-query-devtools`](https://www.npmjs.com/package/@tanstack/react-query-devtools) | 5.74.4 | DevTools для React Query | Отладка кэша |
| [`zustand-devtools`](https://www.npmjs.com/package/zustand-devtools) | — | DevTools для zustand | — |

---

## Инфраструктура / dev tools

| Инструмент | Назначение | Почему он |
|------------|------------|------------|
| **Docker + Docker Compose** | Контейнеризация backend/frontend/db/cache | Унифицированный запуск в dev и prod |
| **PostgreSQL 17.6** | Основная БД | Row-level security, JSONB, full-text search |
| **Redis 8.0** | Кэш, сессии, pub/sub, job queue backend | Быстрая временная и распределённая память |
| **MinIO** | S3-совместимое хранилище вложений | Self-hosted альтернатива AWS S3 |
| **Nginx / Caddy** | Reverse proxy, статика, TLS | Единая точка входа на порту `19876` |
| **Prometheus + Grafana** | Метрики и дашборды | Стандарт observability |
| **Loki / vector** | Централизованные логи | JSON-логи из tracing-subscriber |

---

## Рекомендуемые стартовые наборы

### Backend Cargo.toml (workspace root)

```toml
[workspace]
members = ["api", "domain", "infrastructure", "cli", "shared"]
resolver = "3"

[workspace.dependencies]
axum = "0.8.9"
tokio = { version = "1.52.3", features = ["full"] }
tower = "0.5.2"
tower-http = "0.7.0"
sea-orm = { version = "2.0", features = ["sqlx-postgres", "runtime-tokio-rustls", "macros", "with-chrono", "with-uuid"] }
sqlx = { version = "0.9.0", features = ["runtime-tokio", "postgres", "chrono", "uuid"] }
serde = { version = "1.0.219", features = ["derive"] }
serde_json = "1.0.140"
chrono = { version = "0.4.41", features = ["serde"] }
uuid = { version = "1.17.0", features = ["v4", "v7", "serde"] }
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.19", features = ["env-filter", "json"] }
thiserror = "2.0.12"
anyhow = "1.0.98"
figment = { version = "0.10.19", features = ["env", "toml"] }
garde = "0.23.0"
argon2 = "0.6.0-pre.1"
jsonwebtoken = "10.4.0"
lettre = { version = "0.11.22", features = ["tokio1-native-tls"] }
reqwest = { version = "0.13.4", features = ["json", "rustls-tls"] }
moka = { version = "0.12.15", features = ["future"] }
redis = { version = "1.3.0", features = ["tokio-comp", "connection-manager"] }
utoipa = "5.5.0"
utoipa-axum = "0.2.0"
utoipa-swagger-ui = "9.0.1"
apalis = { version = "0.7.4", features = ["postgres", "tokio-comp"] }
metrics = "0.24.6"
metrics-exporter-prometheus = "0.17.0"
tower_governor = "0.8.0"
tokio-tungstenite = "0.26.2"
mockall = "0.15.0"
testcontainers = { version = "0.27.3", features = ["postgres"] }
wiremock = "0.6.3"
tokio-test = "0.4.4"
rstest = "0.25.0"
```

### Frontend package.json (starter)

```json
{
  "dependencies": {
    "react": "19.1.0",
    "react-dom": "19.1.0",
    "react-router": "8.1.0",
    "@tanstack/react-query": "5.74.4",
    "zustand": "5.0.3",
    "react-hook-form": "7.55.0",
    "zod": "4.4.3",
    "@hookform/resolvers": "5.0.1",
    "@dnd-kit/core": "6.3.1",
    "@dnd-kit/sortable": "10.0.0",
    "@dnd-kit/utilities": "3.2.2",
    "@tiptap/react": "2.11.7",
    "@tiptap/extension-mention": "2.11.7",
    "@tiptap/extension-task-list": "2.11.7",
    "@tiptap/pm": "2.11.7",
    "date-fns": "4.1.0",
    "react-day-picker": "9.6.0",
    "i18next": "25.0.0",
    "react-i18next": "15.5.0",
    "sonner": "2.0.3",
    "@tanstack/react-virtual": "3.13.6",
    "lucide-react": "0.487.0",
    "clsx": "2.1.1",
    "tailwind-merge": "3.2.0",
    "class-variance-authority": "0.7.1",
    "axios": "1.8.4"
  },
  "devDependencies": {
    "vite": "6.2.0",
    "typescript": "5.9.3",
    "@vitejs/plugin-react-swc": "3.9.0",
    "tailwindcss": "4.1.0",
    "@tailwindcss/vite": "4.1.0",
    "vitest": "4.1.10",
    "@testing-library/react": "16.3.0",
    "@testing-library/jest-dom": "6.6.3",
    "@testing-library/user-event": "14.6.1",
    "@playwright/test": "1.61.1",
    "msw": "2.7.5",
    "@types/react": "19.1.0",
    "@types/react-dom": "19.1.0",
    "@tanstack/react-query-devtools": "5.74.4"
  }
}
```

---

## Выводы

- **Backend:** выбираем **Axum + SeaORM + PostgreSQL + Redis**. SeaORM даёт Spring-подобный опыт (Entity/Repository), Axum — Tower-middleware и DI через `Arc<dyn Trait>`.
- **Frontend:** **React 19 + Vite 6 + Tailwind 4 + shadcn/ui + TanStack Query + Zustand + dnd-kit + Tiptap** — полный набор для Jira-like UI.
- **Testing:** mockall + testcontainers на Rust; Vitest + Playwright на React.
- **Infra:** Docker Compose, PostgreSQL 17.6, Redis 8.0, MinIO, Prometheus/Grafana.

Этот набор покрывает все слои Controller → Mapper → Service → Repository + realtime, кэш, очереди, метрики, тесты.
## References

- `docs/ARCHITECTURE.md`
- `docs/FRONTEND_ARCHITECTURE.md`
