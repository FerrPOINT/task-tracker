# Task Tracker

Самостоятельный таск-трекер с канбан-доской. Форк [Task Tracker](https://github.com/go-task-tracker/task-tracker), очищенный и урезанный до MVP.

## Состояние

Работает: проекты, канбан, задачи, фильтры, уведомления.  
Удалены: команды, OAuth/OpenID, CalDAV, вебхуки, тайм-трекинг, импорты из сторонних сервисов, Unsplash, Sentry, метрики.

## Запуск

```bash
cd /opt/dev/task-tracker-next
docker compose up -d
```

- API/UI: http://192.168.1.135:19875
- Порт меняется в `docker-compose.yml` (хост) и `TASKTRACKER_SERVICE_INTERFACE` (контейнер).

## Разработка

```bash
# backend
go build . ./pkg/... ./frontend/...

# frontend
cd frontend
pnpm install
pnpm run build
```

## Структура

- `pkg/routes/api/v1/`, `pkg/routes/api/v2/` — REST API
- `pkg/models/` — модели и бизнес-логика
- `frontend/src/` — Vue + Vite SPA
- `pkg/migration/` — миграции схемы PostgreSQL

## Примечания

- Только PostgreSQL (SQLite/MySQL убраны).
- Язык по умолчанию — русский, fallback — английский.
- Все апстрим-ссылки и брендинг Task Tracker удалены из user-facing поверхности.
