# CLI — Task Tracker

Консольный клиент для работы с API. Бинарник: `task-tracker`.

## Установка

```bash
cd backend
cargo install --path cli
```

## Глобальные флаги

```
--api-url   Базовый URL API (env: TASKTRACKER_API_URL, default: http://localhost:19876)
--token     JWT access token (env: TASKTRACKER_TOKEN)
```

## Аутентификация

```bash
task-tracker auth login --email user@example.com --password secret
task-tracker auth logout
task-tracker auth whoami
```

## Проекты

```bash
task-tracker project list
task-tracker project create --key PROJ --name "Project Name"
task-tracker project get PROJ
task-tracker project update PROJ --name "New Name"
task-tracker project delete PROJ
```

## Задачи

```bash
task-tracker issue create --project-key PROJ --summary "Fix bug" --issue-type task
task-tracker issue get PROJ-1
task-tracker issue update PROJ-1 --summary "Updated" --status-id <uuid>
task-tracker issue delete PROJ-1
task-tracker issue transition PROJ-1 --to <status-uuid>
```

## Примечания

- Токен передаётся через `--token` или переменную `TASKTRACKER_TOKEN`.
- Парсинг ключей (`project get PROJ`, `issue get PROJ-1`) происходит на стороне сервера.
- Остальные команды (миграции, backup, импорт/экспорт, админка) — заглушки в текущей MVP-версии.

## Ссылки

- `docs/API.md`
- `docs/ARCHITECTURE.md`
