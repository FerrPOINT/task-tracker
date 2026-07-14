# Local Setup

> Стартовый документ. До конца разработки команды, зависимости и шаги могут измениться — актуализировать при появлении рабочего backend/frontend.

## 1. Требования

| Инструмент | Минимальная версия | Примечание |
|---|---|---|
| Docker + Compose | 24.x | для Postgres, Redis, Traefik |
| Rust | 1.80+ | backend |
| cargo | 1.80+ | backend |
| Node.js | 22 LTS | frontend |
| pnpm | 9.x | frontend package manager |
| just | — | task runner (опционально) |
| git | 2.40+ | — |

## 2. Быстрый старт

```bash
git clone git@github.com:FerrPOINT/task-tracker.git /opt/dev/task-tracker
cd /opt/dev/task-tracker

cp .env.example .env
# отредактируй .env под себя

docker compose up -d postgres redis
cd backend && cargo run --bin server
cd frontend && pnpm install && pnpm dev
```

Приложение доступно по `http://localhost:19876`.

## 3. Переменные окружения

Основные для локальной разработки:

```env
TASKTRACKER_DATABASE_URL=postgres://tasktracker:[CHANGE_ME]@localhost:5432/tasktracker
TASKTRACKER_REDIS_URL=redis://localhost:6379
TASKTRACKER_JWT_SECRET=[CHANGE_ME_32BYTES_MIN]
TASKTRACKER_REFRESH_SECRET=[CHANGE_ME_32BYTES_MIN]
TASKTRACKER_ADMIN_EMAIL=admin@example.com
TASKTRACKER_ADMIN_PASSWORD=[CHANGE_ME]
VITE_API_URL=/api/v1
VITE_WS_URL=/ws/v1
```

Полный список — в `.env.example`.

## 4. Backend

```bash
cd backend

# Установка зависимостей
cargo build

# Запуск миграций
cargo run --bin migrator

# Запуск API сервера
cargo run --bin server

# Запуск тестов
cargo test

# Запуск с watch
cargo watch -x run --bin server
```

## 5. Frontend

```bash
cd frontend

pnpm install
pnpm dev

# Типизация
pnpm typecheck

# Линтер
pnpm lint

# Тесты
pnpm test:unit
pnpm test:e2e
```

## 6. Docker

```bash
# Всё через compose
docker compose up -d --build

# Только инфраструктура
docker compose up -d postgres redis

# Пересоздать контейнеры после изменений
docker compose build
docker compose up -d

# Логи
docker compose logs -f api
```

## 7. Тестовые данные

После первого запуска:

```bash
# Автосоздание admin пользователя из .env
./scripts/init-admin.sh

# Seed demo-проекта и задач (опционально)
./scripts/seed-demo.sh
```

## 8. IDE

Рекомендуемые расширения:

- Rust Analyzer
- Tailwind CSS IntelliSense
- ESLint
- Prettier
- GitLens
- Docker

## 9. Частые проблемы

| Проблема | Решение |
|---|---|
| Порт 19876 занят | `TASKTRACKER_SERVER_PORT` в `.env` / `docker-compose.override.yml` |
| Postgres не стартует | `docker compose down -v` и пересоздать volume |
| Redis connection refused | проверить `TASKTRACKER_REDIS_URL` |
| `cargo` долго компилирует | `sccache` + `cargo nextest` |

Больше диагностики — в `docs/TROUBLESHOOTING.md`.

## 10. Pre-commit

```bash
# Установить hooks (после создания)
just install-hooks
# или
pre-commit install
```

## 11. References

- `.env.example`
- `docker-compose.yml`
- `docs/DEPLOYMENT.md`
- `docs/TESTING.md`
- `docs/TROUBLESHOOTING.md`
- `docs/CODE_STYLE.md`
- `docs/AGENTS.md`
