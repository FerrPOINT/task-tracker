# FAQ

> Стартовый документ. До конца разработки вопросы и ответы будут пополняться.

## 1. Общие вопросы

### Что это за проект?

Self-hosted таск-трекер, заточенный под kanban и issue-tracking в стиле Jira, но без лишнего веса.

### Почему не используем готовый Jira / Trello / Notion?

Цель — полный контроль над данными, кастомная JQL, workflow engine и интеграции в локальной инфраструктуре.

### Какой лицензии проект?

См. файл лицензии в корне репозитория.

## 2. Архитектура

### Почему Rust + Axum, а не Go / Node / Django?

Архитектурное решение зафиксировано в `docs/adr/0001-rust-axum.md`.

### Почему PostgreSQL?

См. `docs/adr/0003-postgresql.md`.

### Почему Feature-Sliced Design во frontend?

См. `docs/adr/0005-feature-sliced-design.md`.

## 3. Разработка

### Где взять `.env`?

```bash
cp .env.example .env
```

Подробнее в `docs/LOCAL_SETUP.md`.

### Как запустить backend и frontend локально?

```bash
docker compose up -d postgres redis
cd backend && cargo run --bin server
cd frontend && pnpm dev
```

См. `docs/LOCAL_SETUP.md`.

### Как накатить миграции?

```bash
cd backend
cargo run --bin migrator
```

### Как добавить новый endpoint?

1. Описать dto в `backend/src/api/v1/...`.
2. Добавить handler.
3. Зарегистрировать в router.
4. Обновить `docs/API.md` и OpenAPI tags.

### Как добавить фичу во frontend?

1. Определить slice в FSD: `entities/`, `features/`, `widgets/`.
2. Добавить TanStack Query hook в `features/<name>/api/`.
3. UI-компоненты — в `features/<name>/ui/`.
4. Страница — в `pages/`.

## 4. Данные

### Где физическая модель?

`docs/DATA_MODEL.md`.

### Можно ли использовать SQLite?

Нет, проект заточен под PostgreSQL. JQL, JSONB, window functions и workflow требуют PG.

### Как бэкапить?

См. `docs/BACKUP_RESTORE.md`.

## 5. Auth

### Есть ли OAuth / SSO?

Будет реализовано позже. Базовая версия — email + password + JWT.

### Как сменить пароль админа?

```bash
./scripts/reset-admin-password.sh   # скрипт появится при реализации
```

Или через UI: Profile → Security → Change password.

### Где хранятся refresh-токены?

httpOnly cookie + хеш в `refresh_tokens` (см. `docs/DATA_MODEL.md`).

## 6. Deployment

### Какой порт по умолчанию?

`19876` — основной HTTP/HTTPS.

### Как сменить порт?

В `.env` и `docker-compose.override.yml` (compose-файлы появятся при реализации).

### Как обновить на новую версию?

```bash
git pull origin main
docker compose build
docker compose up -d
docker compose run --rm migrator
```

## 7. Troubleshooting

### Где искать, если что-то не работает?

- Логи: `docker compose logs -f api`
- Health: `curl /health/ready`
- Диагностика: `docs/TROUBLESHOOTING.md`

## 8. References

- `docs/LOCAL_SETUP.md`
- `docs/DEPLOYMENT.md`
- `docs/ARCHITECTURE.md`
- `docs/ADR.md`
- `docs/TROUBLESHOOTING.md`
